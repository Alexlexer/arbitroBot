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
    notifier: Arc<TelegramNotifier>,
    // Symbol -> Exchange -> Ticker
    market_data: HashMap<String, HashMap<ExchangeId, UnifiedTicker>>,
}

impl Aggregator {
    pub fn new(
        rx: Receiver<UnifiedTicker>, 
        exec_tx: Sender<ArbitrageOpportunity>,
        log_buffer: Arc<Mutex<VecDeque<String>>>,
        risk_manager: RiskManager,
        funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
        market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
        notifier: Arc<TelegramNotifier>,
    ) -> Self {
        Self {
            rx,
            exec_tx,
            log_buffer,
            risk_manager,
            funding_rates,
            market_filters,
            notifier,
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
                }
            }
        }
    }

    async fn update_market_data(&mut self, ticker: UnifiedTicker) {
        let symbol = ticker.symbol.clone();
        let entry = self.market_data.entry(symbol.clone()).or_insert_with(HashMap::new);
        entry.insert(ticker.exchange, ticker);
        
        // Check for immediate opportunity on every update (High-Frequency style)
        // For simplicity in this step, we keep the visual matrix print separate, 
        // but we can add detection logic here or in a separate method called here.
        self.detect_sharps(&symbol).await;
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
                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); // 0.1% buffer

                    let total_cost_pct = (fee_long + fee_short + slippage) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    // User requested 5.0%+ spread
                    let threshold = Decimal::from(5); // 5.0%
                    
                    if net_spread >= threshold {
                        // Prepare Risk Data
                        let target_volume = Decimal::from(1000); // 1000 USDT target
                        
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

                        // Validate
                        match self.risk_manager.validate(&ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            long_exchange: long.exchange,
                            short_exchange: short.exchange,
                            long_price: b_long.0,
                            short_price: b_short.0,
                            spread_pct: net_spread,
                            timestamp: chrono::Utc::now().timestamp_millis(),
                        }, &depth_map, &funding_map, &status_map, &r_filters, &ticker_timestamps, target_volume).await {
                            Ok(_) => {
                                let opp = ArbitrageOpportunity {
                                    symbol: symbol.to_string(),
                                    long_exchange: long.exchange,
                                    short_exchange: short.exchange,
                                    long_price: b_long.0,
                                    short_price: b_short.0,
                                    spread_pct: net_spread,
                                    timestamp: chrono::Utc::now().timestamp_millis(),
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

        let mut opportunities = Vec::new();

        for (symbol, exchanges) in &self.market_data {
            if exchanges.len() < 2 { continue; }

            let best_long = exchanges.values()
                .min_by(|a, b| a.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX).partial_cmp(&b.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX)).unwrap_or(std::cmp::Ordering::Equal));
            
            let best_short = exchanges.values()
                .max_by(|a, b| a.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN).partial_cmp(&b.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN)).unwrap_or(std::cmp::Ordering::Equal));

            if let (Some(long), Some(short)) = (best_long, best_short) {
                if long.exchange == short.exchange { continue; }

                if let (Some(b_long), Some(b_short)) = (long.best_ask(), short.best_bid()) {
                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); // 0.1% buffer
                    
                    let total_cost_pct = (fee_long + fee_short + slippage) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    if gross_spread > Decimal::from(-1) { 
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

        // Limit table to 10 rows to fit screen
        for (sym, l_ex, l_p, s_ex, s_p, gross, net) in opportunities.into_iter().take(10) {
            let status = if net > Decimal::from(0) { "PROFITABLE" } else { "Loss" };
            table.add_row(Row::new(vec![
                Cell::new(&sym),
                Cell::new(&format!("{} @ {:.4}", l_ex, l_p)), // Compact numbers
                Cell::new(&format!("{} @ {:.4}", s_ex, s_p)),
                Cell::new(&format!("{:.2}%", gross)),
                Cell::new(&format!("{:.2}%", net)),
                Cell::new(status),
            ]));
        }

        table.printstd();

        // Position Logs at fixed line (e.g., line 16)
        // Table (1header + 10rows + 2separators) takes ~14 lines.
        let _ = execute!(stdout, MoveTo(0, 16));
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
}
