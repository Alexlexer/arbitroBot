use crate::model::{ExchangeId, UnifiedTicker, ArbitrageOpportunity, FundingInfo, AssetStatus};
use crate::risk_manager::RiskManager;
use crate::notifier::TelegramNotifier;
use log::{info, error};
use rust_decimal::Decimal;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};

pub struct Aggregator {
    rx: Receiver<UnifiedTicker>,
    exec_tx: Sender<ArbitrageOpportunity>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    risk_manager: RiskManager,
    funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
    market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
    account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    rebalance_advisor: crate::rebalance_advisor::RebalanceAdvisor,
    last_rebalance_advice: Arc<Mutex<Vec<crate::model::RebalanceAdvice>>>,
    notifier: Arc<TelegramNotifier>,
    messaging: crate::messaging::SharedMessaging,
    // Symbol -> Exchange -> Ticker
    market_data: HashMap<String, HashMap<ExchangeId, UnifiedTicker>>,
}

impl Aggregator {
    pub fn new(
        rx: Receiver<UnifiedTicker>, 
        exec_tx: Sender<ArbitrageOpportunity>,
        log_buffer: Arc<Mutex<VecDeque<String>>>,
        risk_manager: RiskManager,
        rebalance_advisor: crate::rebalance_advisor::RebalanceAdvisor,
        funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
        market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
        account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
        notifier: Arc<TelegramNotifier>,
        messaging: crate::messaging::SharedMessaging,
    ) -> Self {
        Self {
            rx,
            exec_tx,
            log_buffer,
            risk_manager,
            funding_rates,
            market_filters,
            account_state,
            rebalance_advisor,
            last_rebalance_advice: Arc::new(Mutex::new(Vec::new())),
            notifier,
            messaging,
            market_data: HashMap::new(),
        }
    }

    pub async fn run(&mut self) {
        info!("Aggregator started.");
        
        // We can use a tick interval to print the matrix periodically
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));

        loop {
            tokio::select! {
                Some(ticker) = self.rx.recv() => {
                    self.update_market_data(ticker).await;
                }
                _ = interval.tick() => {
                    self.print_arbitrage_matrix();
                    self.check_rebalancing().await;
                    self.broadcast_state().await;
                }
            }
        }
    }

    async fn update_market_data(&mut self, ticker: UnifiedTicker) {
        let symbol = ticker.symbol.clone();
        let entry = self.market_data.entry(symbol.clone()).or_insert_with(HashMap::new);
        entry.insert(ticker.exchange, ticker.clone());
        
        // Check for immediate opportunity on every update (High-Frequency style)
        self.detect_sharps(&symbol).await;
        
        // Broadcast ticker update
        self.broadcast_message(&format!("ticker.{}", ticker.exchange), &ticker).await;
    }
    
    async fn detect_sharps(&self, symbol: &str) {
        if let Some(exchanges) = self.market_data.get(symbol) {
            if exchanges.len() < 2 { return; }

            let best_long = exchanges.values()
                .min_by(|a, b| a.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX).partial_cmp(&b.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX)).unwrap_or(std::cmp::Ordering::Equal));
            
            let best_short = exchanges.values()
                .max_by(|a, b| a.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN).partial_cmp(&b.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN)).unwrap_or(std::cmp::Ordering::Equal));

            if let (Some(long), Some(short)) = (best_long, best_short) {
                if long.exchange == short.exchange { return; }

                if let (Some(b_long), Some(b_short)) = (long.best_ask(), short.best_bid()) {
                    let floor = Decimal::new(1, 4); // 0.0001
                    if b_long.0 < floor || b_short.0 < floor { return; }

                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from(2) { return; }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long_total = long.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let fee_short_total = short.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let slippage_total = Decimal::new(2, 3); // 0.2% total buffer (0.1% entry + 0.1% exit)

                    let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    // User requested 5.0%+ spread
                    let threshold = Decimal::from(5); // 5.0%
                    let max_sanity = Decimal::from(50); // 50% max
                    
                    if net_spread >= threshold && net_spread < max_sanity {
                        // Prepare Risk Data
                        let target_volume = Decimal::from(1000); // 1000 USDT target
                        
                        // Rebalancing Transfer Costs: must make at least $2 net profit after spread
                        let expected_profit_usdt = target_volume * (net_spread / Decimal::from(100));
                        if expected_profit_usdt <= Decimal::from(2) { return; }
                        
                        let mut depth_map = HashMap::new();
                        depth_map.insert(long.exchange, crate::model::OrderBookDepth { bids: long.bids.clone(), asks: long.asks.clone() });
                        depth_map.insert(short.exchange, crate::model::OrderBookDepth { bids: short.bids.clone(), asks: short.asks.clone() });

                        let r_rates = self.funding_rates.lock().unwrap();
                        let mut funding_map = HashMap::new();
                        if let Some(exchange_rates) = r_rates.get(&long.exchange) {
                            if let Some(info) = exchange_rates.get(symbol) {
                                funding_map.insert(long.exchange, info.clone());
                            }
                        }
                        if let Some(exchange_rates) = r_rates.get(&short.exchange) {
                            if let Some(info) = exchange_rates.get(symbol) {
                                funding_map.insert(short.exchange, info.clone());
                            }
                        }

                        // Wallet status (Mocked for now as all active)
                        let mut status_map = HashMap::new();
                        status_map.insert(long.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
                        status_map.insert(short.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });

                        let r_filters = self.market_filters.lock().unwrap();
                        
                        let mut ticker_timestamps = HashMap::new();
                        ticker_timestamps.insert(long.exchange, long.timestamp);
                        ticker_timestamps.insert(short.exchange, short.timestamp);

                        let account_state = self.account_state.lock().unwrap();

                        // Validate
                        match self.risk_manager.validate(&ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            long_exchange: long.exchange,
                            short_exchange: short.exchange,
                            long_price: b_long.0,
                            short_price: b_short.0,
                            spread_pct: net_spread,
                            _timestamp: chrono::Utc::now().timestamp_millis(),
                        }, &depth_map, &funding_map, &status_map, &r_filters, &ticker_timestamps, &account_state, target_volume).await {
                            Ok(_) => {
                                let opp = ArbitrageOpportunity {
                                    symbol: symbol.to_string(),
                                    long_exchange: long.exchange,
                                    short_exchange: short.exchange,
                                    long_price: b_long.0,
                                    short_price: b_short.0,
                                    spread_pct: net_spread,
                                    _timestamp: chrono::Utc::now().timestamp_millis(),
                                };

                                if let Err(e) = self.exec_tx.send(opp).await {
                                    error!("Failed to send opportunity to ExecutionActor: {}", e);
                                }

                                // Send Telegram Alert
                                let msg = format!(
                                    "🚀 *Arbitrage Opportunity Found!*\n\n\
                                    *Symbol*: {}\n\
                                    *Long*: {} @ {:.4}\n\
                                    *Short*: {} @ {:.4}\n\
                                    *Net Spread*: {:.2}%\n\
                                    *Target*: $1000",
                                    symbol, long.exchange, b_long.0, short.exchange, b_short.0, net_spread
                                );
                                let n = self.notifier.clone();
                                tokio::spawn(async move {
                                    n.send_alert(&msg).await;
                                });
                            },
                            Err(e) => {
                                info!("Risk Check Failed for {}: {}", symbol, e);
                            }
                        }
                    }
                }
            }
        }
    }

    fn print_arbitrage_matrix(&self) {
        use crossterm::{execute, terminal::{Clear, ClearType}, cursor::{MoveTo, Hide}};
        use std::io::stdout;
        use prettytable::{Table, Row, Cell, format};

        let mut stdout = stdout();
        
        // 1. Reset Cursor to Top
        // We use ClearType::All to ensure we wipe the slate clean every frame.
        let _ = execute!(stdout, Hide, Clear(ClearType::All), MoveTo(0, 0));

        println!("=== ARBITRAGE MATRIX (Threshold: 5.0%+) ===");
        println!("Last Update: {}", chrono::Local::now().format("%H:%M:%S"));

        // 2. Plot Equity Summary
        if let Ok(state) = self.account_state.lock() {
            println!("\n[ BALANCE MONITOR ]");
            println!("Total Equity: ${:.2} USDT | Global PnL: ${:.2}", 
                state.total_equity_usdt, state.total_unrealized_pnl);
            
            let mut summary = String::new();
            for (ex, s) in &state.exchange_states {
                summary.push_str(&format!("{}: ${:.1} ", ex, s.total_equity));
            }
            println!("Exchanges: {}", summary);
            println!("--------------------------------------------------");
        }

        let mut opportunities = Vec::new();

        for (symbol, exchanges) in &self.market_data {
            if exchanges.len() < 2 { continue; }

            // Trading Status Check
            {
                let filters = self.market_filters.lock().unwrap();
                let mut all_tradable = true;
                for (eid, _) in exchanges {
                    if let Some(ef) = filters.get(eid) {
                        if let Some(f) = ef.get(symbol) {
                            if !f.is_trading {
                                all_tradable = false;
                                break;
                            }
                        }
                    }
                }
                if !all_tradable { continue; }
            }

            let best_long = exchanges.values()
                .min_by(|a, b| a.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX).partial_cmp(&b.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX)).unwrap_or(std::cmp::Ordering::Equal));
            
            let best_short = exchanges.values()
                .max_by(|a, b| a.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN).partial_cmp(&b.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN)).unwrap_or(std::cmp::Ordering::Equal));

            if let (Some(long), Some(short)) = (best_long, best_short) {
                if long.exchange == short.exchange { continue; }

                if let (Some(b_long), Some(b_short)) = (long.best_ask(), short.best_bid()) {
                    let floor = Decimal::new(1, 4); // 0.0001 USDT floor
                    if b_long.0 < floor || b_short.0 < floor { continue; }

                    // Magnitude Check: Filter out unit mismatches (1:1000 etc) or different coins
                    // Price ratio > 2.0x difference is almost always a unit or coin mismatch
                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from(2) { continue; }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long_total = long.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let fee_short_total = short.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let slippage_total = Decimal::new(2, 3); // 0.2% total buffer (0.1% entry + 0.1% exit)
                    
                    let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    if gross_spread > Decimal::from(-1) && gross_spread < Decimal::from(50) { 
                        opportunities.push((
                            symbol.clone(),
                            long.exchange, b_long.0,
                            short.exchange, b_short.0,
                            gross_spread,
                            net_spread
                        ));
                    }
                }
            }
        }

        opportunities.sort_by(|a, b| b.6.partial_cmp(&a.6).unwrap_or(std::cmp::Ordering::Equal));

        let mut table = Table::new();
        table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);
        table.set_titles(Row::new(vec![
            Cell::new("Symbol"),
            Cell::new("Long"),
            Cell::new("Short"),
            Cell::new("Gross %"),
            Cell::new("Net %"),
            Cell::new("Status"),
        ]));

        // Limit table to 5 rows to avoid UI overlap
        for (sym, l_ex, l_p, s_ex, s_p, gross, net) in opportunities.into_iter().take(5) {
            let status = if net > Decimal::from(0) { "PROFITABLE" } else { "Loss" };
            table.add_row(Row::new(vec![
                Cell::new(&sym),
                Cell::new(&format!("{} @ {:.4}", l_ex, l_p)),
                Cell::new(&format!("{} @ {:.4}", s_ex, s_p)),
                Cell::new(&format!("{:.2}%", gross)),
                Cell::new(&format!("{:.2}%", net)),
                Cell::new(status),
            ]));
        }

        table.printstd();

        // 3. Show Rebalance Advice
        if let Ok(advices) = self.last_rebalance_advice.lock() {
            if !advices.is_empty() {
                println!("\n⚠️  REBALANCE REQUIRED:");
                for a in advices.iter() {
                    println!("   - Move ${:.0} from {} to {} ({})", a.amount_usdt, a.from_exchange, a.to_exchange, a.reason);
                }
            }
        }

        // Position Logs at fixed line (e.g., line 12)
        // Header + balance + 5 rows + 1 advice = ~12 lines
        let _ = execute!(stdout, MoveTo(0, 14));
        println!("=== RECENT ACTIVITY (Last 5) ===");
        if let Ok(logs) = self.log_buffer.lock() {
            let start = if logs.len() > 5 { logs.len() - 5 } else { 0 };
            for log in logs.iter().skip(start) {
                // Truncate log line to avoid wrap-around
                let log_trim = if log.len() > 80 { &log[..80] } else { log };
                println!("{}", log_trim);
            }
        }
        
        // Clear anything below logs
        let _ = execute!(stdout, Clear(ClearType::FromCursorDown));
    }

    async fn check_rebalancing(&self) {
        let state = {
            let s = self.account_state.lock().unwrap();
            s.clone()
        };

        let advices = self.rebalance_advisor.check(&state);
        
        // Store for TUI
        {
            let mut last = self.last_rebalance_advice.lock().unwrap();
            *last = advices.clone();
        }

        for advice in advices {
            let msg = format!(
                "⚖️ *Rebalance Suggestion*\n\n\
                *Reason*: {}\n\
                *Action*: Move `${:.0} USDT` from **{}** to **{}**",
                advice.reason, advice.amount_usdt, advice.from_exchange, advice.to_exchange
            );
            
            // Log locally
            info!("REBALANCE: {}", advice.reason);
            
            // Notify Telegram
            let n = self.notifier.clone();
            tokio::spawn(async move {
                n.send_alert(&msg).await;
            });
        }
    }

    async fn broadcast_state(&self) {
        let state = self.account_state.lock().unwrap().clone();
        self.broadcast_message("account.state", &state).await;
    }

    async fn broadcast_message<T: serde::Serialize>(&self, routing_key: &str, payload: &T) {
        let msg = self.messaging.lock().await;
        if let Some(client) = msg.as_ref() {
            if let Err(e) = client.publish(routing_key, payload).await {
                error!("RabbitMQ Publish Error ({}): {}", routing_key, e);
            }
        }
    }
}
