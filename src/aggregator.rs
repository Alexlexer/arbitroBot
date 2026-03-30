use crate::model::{ExchangeId, UnifiedTicker, ArbitrageOpportunity, FundingInfo, AssetStatus};
use crate::risk_manager::RiskManager;
use crate::notifier::TelegramNotifier;
use log::{info, error};
use rust_decimal::Decimal;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use amqprs::consumer::AsyncConsumer;
use amqprs::{Deliver, BasicProperties};
use amqprs::channel::BasicAckArguments;
use async_trait::async_trait;

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

    pub fn get_command_consumer(&self) -> CommandConsumer {
        CommandConsumer {
            config: self.risk_manager.config.clone(),
            secrets: self.risk_manager.secrets.clone(),
            notifier: self.notifier.clone(),
        }
    }

    pub async fn run(&mut self) {
        info!("Aggregator started.");
        
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
        
        self.detect_sharps(&symbol).await;
        
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
                    let floor = Decimal::new(1, 4); 
                    if b_long.0 < floor || b_short.0 < floor { return; }

                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from(2) { return; }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); 

                    let total_cost_pct = (fee_long + fee_short + slippage) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    let threshold = Decimal::from(5); 
                    let max_sanity = Decimal::from(50); 
                    
                    if net_spread >= threshold && net_spread < max_sanity {
                        let target_volume = Decimal::from(1000); 
                        
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

                        let mut status_map = HashMap::new();
                        status_map.insert(long.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
                        status_map.insert(short.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });

                        let r_filters = self.market_filters.lock().unwrap();
                        
                        let mut ticker_timestamps = HashMap::new();
                        ticker_timestamps.insert(long.exchange, long.timestamp);
                        ticker_timestamps.insert(short.exchange, short.timestamp);

                        let account_state_clone = {
                            let s = self.account_state.lock().unwrap();
                            s.clone()
                        };

                        match self.risk_manager.validate(&ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            long_exchange: long.exchange,
                            short_exchange: short.exchange,
                            long_price: b_long.0,
                            short_price: b_short.0,
                            spread_pct: net_spread,
                            _timestamp: chrono::Utc::now().timestamp_millis(),
                        }, &depth_map, &funding_map, &status_map, &r_filters, &ticker_timestamps, &account_state_clone, target_volume).await {
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
                                let msg_owned = msg.clone();
                                tokio::spawn(async move {
                                    n.send_alert(&msg_owned).await;
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
        let _ = execute!(stdout, Hide, Clear(ClearType::All), MoveTo(0, 0));

        println!("=== ARBITRAGE MATRIX (Threshold: 5.0%+) ===");
        println!("Last Update: {}", chrono::Local::now().format("%H:%M:%S"));

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
                    let floor = Decimal::new(1, 4); 
                    if b_long.0 < floor || b_short.0 < floor { continue; }

                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from(2) { continue; }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); 
                    
                    let total_cost_pct = (fee_long + fee_short + slippage) * Decimal::from(100);
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

        if let Ok(advices) = self.last_rebalance_advice.lock() {
            if !advices.is_empty() {
                println!("\n\u{26A0}\u{FE0F}  REBALANCE REQUIRED:");
                for a in advices.iter() {
                    println!("   - Move ${:.0} from {} to {} ({})", a.amount_usdt, a.from_exchange, a.to_exchange, a.reason);
                }
            }
        }

        let _ = execute!(stdout, MoveTo(0, 14));
        println!("=== RECENT ACTIVITY (Last 5) ===");
        if let Ok(logs) = self.log_buffer.lock() {
            let start = if logs.len() > 5 { logs.len() - 5 } else { 0 };
            for log in logs.iter().skip(start) {
                let log_trim = if log.len() > 80 { &log[..80] } else { log };
                println!("{}", log_trim);
            }
        }
        
        let _ = execute!(stdout, Clear(ClearType::FromCursorDown));
    }

    async fn check_rebalancing(&self) {
        let state = {
            let s = self.account_state.lock().unwrap();
            s.clone()
        };

        let advices = self.rebalance_advisor.check(&state);
        
        {
            let mut last = self.last_rebalance_advice.lock().unwrap();
            *last = advices.clone();
        }

        for advice in advices {
            info!("REBALANCE ADVICE: From {} to {} Amount {}", advice.from_exchange, advice.to_exchange, advice.amount_usdt);
            
            let n = self.notifier.clone();
            let advice_owned = advice.clone();
            tokio::spawn(async move {
                n.send_rebalance_confirmation(&advice_owned).await;
            });
        }
    }

    async fn broadcast_state(&self) {
        let mut state = self.account_state.lock().unwrap().clone();
        
        // Mask Secrets before broadcasting
        let s = &mut state.secrets;
        let mask = |o: &mut Option<String>| {
            if let Some(val) = o {
                if val.len() > 8 {
                    *o = Some(format!("{}...{}", &val[..4], &val[val.len()-4..]));
                } else {
                    *o = Some("********".to_string());
                }
            }
        };

        mask(&mut s.binance_key); mask(&mut s.binance_secret);
        mask(&mut s.bybit_key); mask(&mut s.bybit_secret);
        mask(&mut s.bitget_key); mask(&mut s.bitget_secret); mask(&mut s.bitget_passphrase);
        mask(&mut s.mexc_key); mask(&mut s.mexc_secret);
        mask(&mut s.okx_key); mask(&mut s.okx_secret); mask(&mut s.okx_passphrase);
        mask(&mut s.telegram_token); mask(&mut s.telegram_chat_id);

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

pub struct CommandConsumer {
    config: Arc<Mutex<crate::config::AppConfig>>,
    secrets: Arc<Mutex<crate::config::SecretsConfig>>,
    notifier: Arc<crate::notifier::TelegramNotifier>,
}

#[async_trait]
impl AsyncConsumer for CommandConsumer {
    async fn consume(
        &mut self,
        channel: &amqprs::channel::Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        if let Ok(cmd) = serde_json::from_slice::<crate::model::BotCommand>(&content) {
            info!("COMMAND_RECEIVED: {:?}", cmd);
            match cmd {
                crate::model::BotCommand::UpdateConfig(new_conf) => {
                    let mut conf = self.config.lock().unwrap();
                    *conf = new_conf;
                    let _ = conf._save();
                    info!("CONFIG_UPDATED: New thresholds applied.");
                }
                crate::model::BotCommand::UpdateSecrets(new_secrets) => {
                    let mut sec = self.secrets.lock().unwrap();
                    *sec = new_secrets;
                    let _ = sec.save();
                    info!("SECRETS_UPDATED: API credentials refreshed.");
                }
                crate::model::BotCommand::EmergencyStop => {
                    let mut conf = self.config.lock().unwrap();
                    conf.automated_rebalance_enabled = false; 
                    info!("EMERGENCY_STOP: Automated rebalance disabled.");
                    let n = self.notifier.clone();
                    tokio::spawn(async move {
                        n.send_alert("🛑 *EMERGENCY STOP* triggered from Dashboard. Automated rebalancing disabled.").await;
                    });
                }
                crate::model::BotCommand::Resume => {
                    let mut conf = self.config.lock().unwrap();
                    conf.automated_rebalance_enabled = true;
                    info!("BOT_RESUMED: Automated rebalance re-enabled.");
                    let n = self.notifier.clone();
                    tokio::spawn(async move {
                        n.send_alert("✅ *BOT RESUMED* from Dashboard. Automated rebalancing re-enabled.").await;
                    });
                }
            }
        }
        let _ = channel.basic_ack(BasicAckArguments::new(deliver.delivery_tag(), false)).await;
    }
}
