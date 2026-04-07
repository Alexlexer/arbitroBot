use crate::constants::{
    TICKER_STALE_MS,
    target_volume_usdt, transfer_fee_usdt, max_slippage_for_safe_volume,
    max_position_percentage,
};
use crate::auth;
use crate::model::{
    ExchangeId, UnifiedTicker, ArbitrageOpportunity, FundingInfo, AssetStatus,
    DashboardAuthResponse, HistoryOpportunity, ListenerAlert, ListenerOpportunityPayload,
};
use crate::risk_manager::RiskManager;
use crate::notifier::TelegramNotifier;
use log::{info, error};
use rust_decimal::Decimal;
use std::str::FromStr;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::watch;
use async_trait::async_trait;
use amqprs::{Deliver, BasicProperties, channel::BasicAckArguments, consumer::AsyncConsumer};

pub struct Aggregator {
    rx: Receiver<UnifiedTicker>,
    command_rx: Receiver<crate::model::BotCommand>,
    exec_tx: Sender<ArbitrageOpportunity>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    risk_manager: RiskManager,
    funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
    market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
    account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    rebalance_advisor: crate::rebalance_advisor::RebalanceAdvisor,
    last_rebalance_advice: Arc<Mutex<Vec<crate::model::RebalanceAdvice>>>,
    last_rebalance_notified: Arc<Mutex<Option<Vec<crate::model::RebalanceAdvice>>>>,
    /// (symbol, long_ex, short_ex) -> last_alert_ts_ms (cooldown to avoid spam)
    last_opportunity_alert: Arc<Mutex<HashMap<String, i64>>>,
    /// Forwarded from Listener WS client (type="alert")
    listener_alert_rx: Receiver<ListenerAlert>,
    /// Updated from BotConfig so Listener WS client can reconnect
    listener_url_tx: watch::Sender<String>,
    notifier: Arc<TelegramNotifier>,
    pub messaging: crate::messaging::SharedMessaging,
    // Symbol -> Exchange -> Ticker
    market_data: HashMap<String, HashMap<ExchangeId, UnifiedTicker>>,
    config: Arc<Mutex<crate::config::AppConfig>>,
    /// Tickers received per exchange (reset every FEED_LOG_INTERVAL)
    ticker_counts: HashMap<ExchangeId, u64>,
    /// Tick count for periodic feed log (every 7 ticks = 14s)
    feed_log_ticks: u32,
    /// Snapshot channel for history (every ~5 min). None if history disabled.
    snapshot_tx: Option<Sender<Vec<HistoryOpportunity>>>,
    /// Tick count for snapshot (every 150 ticks = 5 min)
    snapshot_ticks: u32,
}

impl Aggregator {
    pub fn new(
        rx: Receiver<UnifiedTicker>, 
        command_rx: Receiver<crate::model::BotCommand>,
        exec_tx: Sender<ArbitrageOpportunity>,
        log_buffer: Arc<Mutex<VecDeque<String>>>,
        risk_manager: RiskManager,
        rebalance_advisor: crate::rebalance_advisor::RebalanceAdvisor,
        funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
        market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
        account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
        notifier: Arc<TelegramNotifier>,
        messaging: crate::messaging::SharedMessaging,

        listener_alert_rx: Receiver<ListenerAlert>,
        listener_url_tx: watch::Sender<String>,
        config: Arc<Mutex<crate::config::AppConfig>>,
        snapshot_tx: Option<Sender<Vec<HistoryOpportunity>>>,
    ) -> Self {
        Self {
            rx,
            command_rx,
            exec_tx,
            log_buffer,
            risk_manager,
            funding_rates,
            market_filters,
            account_state,
            rebalance_advisor,
            last_rebalance_advice: Arc::new(Mutex::new(Vec::new())),
            last_rebalance_notified: Arc::new(Mutex::new(None)),
            last_opportunity_alert: Arc::new(Mutex::new(HashMap::new())),
            notifier,
            messaging,
            market_data: HashMap::new(),
            config,
            ticker_counts: HashMap::new(),
            feed_log_ticks: 0,
            snapshot_tx,
            snapshot_ticks: 0,
            listener_alert_rx,
            listener_url_tx,
        }
    }

    pub fn update_config(&mut self, config: crate::config::AppConfig) {
        info!("Updating Aggregator config: Spread Threshold set to {:.2}%", config.min_spread_threshold);
        let mut c = self.config.lock().unwrap_or_else(|e| e.into_inner());
        *c = config;
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
                Some(alert) = self.listener_alert_rx.recv() => {
                    self.handle_listener_alert(alert).await;
                }
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                _ = interval.tick() => {
                    self.feed_log_ticks += 1;
                    if self.feed_log_ticks >= 7 {
                        self.feed_log_ticks = 0;
                        let mut parts: Vec<String> = self.ticker_counts.iter()
                            .map(|(ex, n)| format!("{}: {}", ex, n))
                            .collect();
                        parts.sort();
                        info!("FEED {}", parts.join(", "));
                        self.ticker_counts.clear();
                    }
                    self.snapshot_ticks += 1;
                    if self.snapshot_ticks >= 150 {
                        self.snapshot_ticks = 0;
                        if let Some(ref tx) = self.snapshot_tx {
                            let opportunities = self.current_opportunities(30);
                            let payload: Vec<HistoryOpportunity> = opportunities
                                .into_iter()
                                .map(|(sym, l_ex, l_p, s_ex, s_p, _gross, net)| HistoryOpportunity {
                                    symbol: sym,
                                    long_exchange: format!("{:?}", l_ex),
                                    long_price: l_p.to_string().parse::<f64>().unwrap_or(0.0),
                                    short_exchange: format!("{:?}", s_ex),
                                    short_price: s_p.to_string().parse::<f64>().unwrap_or(0.0),
                                    spread: net.to_string().parse::<f64>().unwrap_or(0.0),
                                })
                                .collect();
                            let _ = tx.try_send(payload);
                        }
                    }
                    self.print_arbitrage_matrix();
                    self.check_rebalancing().await;
                    self.broadcast_tickers().await;
                    self.broadcast_state().await;
                    self.broadcast_config().await;
                }
            }
        }
    }

    async fn handle_listener_alert(&mut self, alert: ListenerAlert) {
        self.broadcast_message("listener.alert", &alert).await;

        if alert.kind.to_lowercase() != "futures" {
            return;
        }

        let symbol_norm = crate::model::normalize_symbol(&alert.symbol);

        // Dynamic subscription: if symbol not in market data, attempt REST fetch
        if !self.market_data.contains_key(&symbol_norm) {
            info!("LISTENER: New symbol {} not in market data — triggering quick REST scan", symbol_norm);
            self.quick_fetch_symbol(&symbol_norm).await;
        }

        let Some(exchanges) = self.market_data.get(&symbol_norm) else {
            info!("LISTENER: Still no data for {} after quick fetch", symbol_norm);
            return;
        };
        if exchanges.len() < 2 {
            return;
        }

        let fee_overrides = {
            let c = self.config.lock().unwrap_or_else(|e| e.into_inner());
            c.taker_fee_overrides.clone()
        };

        let best_long = exchanges
            .values()
            .min_by(|a, b| {
                a.best_ask()
                    .map(|x| x.0)
                    .unwrap_or(Decimal::MAX)
                    .partial_cmp(&b.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        let best_short = exchanges
            .values()
            .max_by(|a, b| {
                a.best_bid()
                    .map(|x| x.0)
                    .unwrap_or(Decimal::MIN)
                    .partial_cmp(&b.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap();

        if best_long.exchange == best_short.exchange {
            return;
        }

        let Some(b_long) = best_long.best_ask() else { return; };
        let Some(b_short) = best_short.best_bid() else { return; };

        let floor = Decimal::from_str("0.00000001").unwrap();
        if b_long.0 <= floor || b_short.0 <= floor {
            return;
        }

        let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);

        let fee_long_total = best_long.exchange.effective_taker_fee(&fee_overrides) * Decimal::from(2);
        let fee_short_total = best_short.exchange.effective_taker_fee(&fee_overrides) * Decimal::from(2);
        let slippage_total = Decimal::new(2, 3);
        let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
        let net_spread = gross_spread - total_cost_pct;

        let payload = ListenerOpportunityPayload {
            symbol: symbol_norm,
            long_exchange: format!("{:?}", best_long.exchange),
            long_price: b_long.0.to_string().parse::<f64>().unwrap_or(0.0),
            short_exchange: format!("{:?}", best_short.exchange),
            short_price: b_short.0.to_string().parse::<f64>().unwrap_or(0.0),
            spread: net_spread.to_string().parse::<f64>().unwrap_or(0.0),
        };
        self.broadcast_message("listener.opportunity", &payload).await;
    }

    /// Quick REST fetch for a specific symbol across major exchanges (for Listener dynamic subscription).
    async fn quick_fetch_symbol(&mut self, symbol: &str) {
        let client = reqwest::Client::new();
        let binance_sym = format!("{}USDT", symbol);
        let bybit_sym = format!("{}USDT", symbol);

        let (b_res, by_res) = tokio::join!(
            Self::fetch_depth_binance(&client, &binance_sym),
            Self::fetch_depth_bybit(&client, &bybit_sym),
        );

        let mut injected = 0u32;
        for result in [b_res, by_res] {
            if let Ok(Some(ticker)) = result {
                let entry = self.market_data.entry(ticker.symbol.clone()).or_insert_with(HashMap::new);
                entry.insert(ticker.exchange, ticker);
                injected += 1;
            }
        }
        if injected > 0 {
            info!("LISTENER: Injected {} exchange snapshots for {}", injected, symbol);
        }
    }

    async fn fetch_depth_binance(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("https://fapi.binance.com/fapi/v1/depth?symbol={}&limit=5", symbol);
        let resp = client.get(&url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        Self::parse_standard_depth(&json, symbol, ExchangeId::Binance)
    }

    async fn fetch_depth_bybit(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("https://api.bybit.com/v5/market/orderbook?category=linear&symbol={}&limit=5", symbol);
        let resp = client.get(&url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let result = json.get("result").cloned().unwrap_or(serde_json::Value::Null);
        Self::parse_standard_depth(&result, symbol, ExchangeId::Bybit)
    }


    fn parse_standard_depth(json: &serde_json::Value, symbol: &str, exchange: ExchangeId) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
        let empty = vec![];
        let bids_raw = json.get("bids").and_then(|b| b.as_array()).unwrap_or(&empty);
        let asks_raw = json.get("asks").and_then(|a| a.as_array()).unwrap_or(&empty);
        let mut bids = Vec::new();
        let mut asks = Vec::new();
        for b in bids_raw.iter().take(5) {
            if let Some(arr) = b.as_array() {
                if arr.len() >= 2 {
                    if let (Ok(p), Ok(q)) = (
                        Decimal::from_str(arr[0].as_str().unwrap_or("")),
                        Decimal::from_str(arr[1].as_str().unwrap_or("")),
                    ) {
                        if p > Decimal::ZERO && q > Decimal::ZERO { bids.push((p, q)); }
                    }
                }
            }
        }
        for a in asks_raw.iter().take(5) {
            if let Some(arr) = a.as_array() {
                if arr.len() >= 2 {
                    if let (Ok(p), Ok(q)) = (
                        Decimal::from_str(arr[0].as_str().unwrap_or("")),
                        Decimal::from_str(arr[1].as_str().unwrap_or("")),
                    ) {
                        if p > Decimal::ZERO && q > Decimal::ZERO { asks.push((p, q)); }
                    }
                }
            }
        }
        if bids.is_empty() || asks.is_empty() {
            return Ok(None);
        }
        Ok(Some(UnifiedTicker {
            symbol: crate::model::normalize_symbol(symbol),
            exchange,
            timestamp: chrono::Utc::now().timestamp_millis(),
            bids,
            asks,
        }))
    }

    async fn handle_command(&mut self, cmd: crate::model::BotCommand) {
        use crate::model::BotCommand;
        let auth_reply = match &cmd {
            BotCommand::DashboardLogin { username, password, request_id } => {
                Some(("login", username.clone(), password.clone(), request_id.clone(), None))
            }
            BotCommand::DashboardRegister { username, password, invite_code, request_id } => {
                Some(("register", username.clone(), password.clone(), request_id.clone(), Some(invite_code.clone())))
            }
            _ => None,
        };
        {
            let mut c = self.config.lock().unwrap_or_else(|e| e.into_inner());
            match cmd {
                BotCommand::UpdateSpread { threshold } => {
                    c.min_spread_threshold = threshold;
                    info!("COMMAND: Spread threshold updated to {:.2}%", threshold);
                }
                BotCommand::UpdateDepth { depth_usdt } => {
                    c.depth_usdt = depth_usdt;
                    info!("COMMAND: Depth (USDT) updated to {}", depth_usdt);
                }
                BotCommand::UpdateListenerWsUrl { url } => {
                    let trimmed = url.trim();
                    c.listener_ws_url = if trimmed.is_empty() {
                        None
                    } else {
                        Some(trimmed.to_string())
                    };
                    let _ = self
                        .listener_url_tx
                        .send(c.listener_ws_url.clone().unwrap_or_default());
                    info!(
                        "COMMAND: Listener WS URL updated (enabled={})",
                        c.listener_ws_url.is_some()
                    );
                }
                BotCommand::ToggleExchange { exchange, enabled } => {
                    c.enabled_exchanges.insert(exchange, enabled);
                    info!("COMMAND: Exchange {} toggled to {}", exchange, enabled);
                }
                BotCommand::UpdateApiKeys { exchange, credentials } => {
                    TelegramNotifier::append_credentials_to_env(
                        exchange,
                        &credentials.key,
                        &credentials.secret,
                        credentials.passphrase.as_deref().unwrap_or(""),
                    );
                    c.api_keys.insert(exchange, credentials);
                    info!("COMMAND: API Keys updated for {}", exchange);
                }
                BotCommand::UpdateFees { exchange, fee } => {
                    c.taker_fee_overrides.insert(exchange, fee);
                    info!("COMMAND: Fee override for {} set to {}", exchange, fee);
                }
                BotCommand::DashboardLogin { .. } | BotCommand::DashboardRegister { .. } => {}
                BotCommand::UpdateBlacklist { symbols } => {
                    info!("COMMAND: Blacklist updated: {:?}", symbols);
                    c.symbol_blacklist = symbols;
                }
                BotCommand::UpdateConfig { config } => {
                    *c = config;
                    info!("COMMAND: Full config updated");
                }
                BotCommand::UpdateSecrets { secrets: new_secrets } => {
                    use crate::config::ExchangeCredentials;
                    use crate::model::ExchangeId;
                    macro_rules! add_key {
                        ($ex:expr, $key:expr, $sec:expr, $pass:expr) => {
                            if let (Some(k), Some(s)) = ($key.clone(), $sec.clone()) {
                                if !k.is_empty() {
                                    c.api_keys.insert($ex, ExchangeCredentials { key: k, secret: s, passphrase: $pass });
                                }
                            }
                        };
                    }
                    add_key!(ExchangeId::Binance,    new_secrets.binance_key,  new_secrets.binance_secret,  None);
                    add_key!(ExchangeId::Bybit,      new_secrets.bybit_key,    new_secrets.bybit_secret,    None);
                    add_key!(ExchangeId::Gate,       new_secrets.gate_key,     new_secrets.gate_secret,     None);
                    // DEX exchanges: private key stored as both key and secret
                    if let Some(pk) = new_secrets.hyperliquid_private_key.clone() {
                        if !pk.is_empty() {
                            c.api_keys.insert(ExchangeId::Hyperliquid, ExchangeCredentials {
                                key: pk.clone(), secret: pk, passphrase: None,
                            });
                        }
                    }
                    if let Some(pk) = new_secrets.aster_private_key.clone() {
                        if !pk.is_empty() {
                            c.api_keys.insert(ExchangeId::Aster, ExchangeCredentials {
                                key: pk.clone(), secret: pk, passphrase: None,
                            });
                        }
                    }
                    if let Err(e) = new_secrets.save() {
                        error!("Failed to save secrets: {}", e);
                    }
                    info!("COMMAND: Secrets updated for {} exchanges", c.api_keys.len());
                }
                BotCommand::EmergencyStop => {
                    info!("COMMAND: Emergency stop received");
                }
                BotCommand::Resume => {
                    info!("COMMAND: Resume received");
                }
            }
            if auth_reply.is_none() {
                let _ = c.save();
            }
        }
        if let Some((action, username, password, request_id, invite_code_opt)) = auth_reply {
            let (ok, error) = if action == "login" {
                let ok = auth::verify_login(&username, &password);
                (ok, None)
            } else {
                match auth::register_user(&username, &password, invite_code_opt.as_deref().unwrap_or("")) {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                }
            };
            let response = DashboardAuthResponse {
                ok,
                request_id,
                username: if ok { Some(username) } else { None },
                error,
            };
            self.broadcast_message("dashboard.auth", &response).await;
            return;
        }
        self.broadcast_config().await;
    }

    async fn update_market_data(&mut self, ticker: UnifiedTicker) {
        *self.ticker_counts.entry(ticker.exchange).or_insert(0) += 1;
        let symbol = ticker.symbol.clone();
        let entry = self.market_data.entry(symbol.clone()).or_insert_with(HashMap::new);
        entry.insert(ticker.exchange, ticker.clone());
        self.detect_sharps(&symbol).await;
        // Tickers are published in bulk via broadcast_tickers() on each interval tick.
    }

    /// Helper: compute VWAP price for given USDT notional on one side of the book.
    /// This is diagnostic only for now; trading still uses top-of-book prices.
    fn vwap_for_volume_usdt(
        &self,
        book: &crate::model::OrderBookDepth,
        side: &str, // "buy" uses asks, "sell" uses bids
        target_usdt: Decimal,
    ) -> Option<(Decimal, Decimal)> {
        if target_usdt <= Decimal::ZERO {
            return None;
        }
        let mut remaining = target_usdt;
        let mut notional = Decimal::ZERO;
        let mut filled_qty = Decimal::ZERO;

        let levels: &Vec<(Decimal, Decimal)> = if side == "buy" {
            &book.asks
        } else {
            &book.bids
        };

        for (price, qty) in levels {
            if *price <= Decimal::ZERO || *qty <= Decimal::ZERO {
                continue;
            }
            let level_usdt = *price * *qty;
            if level_usdt <= Decimal::ZERO {
                continue;
            }
            if level_usdt >= remaining {
                let take_qty = remaining / *price;
                notional += *price * take_qty;
                filled_qty += take_qty;
                // remaining = Decimal::ZERO; // Assignment not needed since we break
                break;
            } else {
                notional += level_usdt;
                filled_qty += *qty;
                remaining -= level_usdt;
            }
        }

        if filled_qty > Decimal::ZERO {
            let vwap = notional / filled_qty;
            let filled_usdt = notional;
            Some((vwap, filled_usdt))
        } else {
            None
        }
    }

    async fn detect_sharps(&self, symbol: &str) {
        // Skip blacklisted symbols (hacked tokens, bad data, etc.)
        {
            let cfg = self.config.lock().unwrap_or_else(|e| e.into_inner());
            if cfg.symbol_blacklist.iter().any(|b| b.eq_ignore_ascii_case(symbol)) {
                return;
            }
        }

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
                    if price_ratio > Decimal::from(11) {
                        return;
                    }

                    // Freshness Check: ignore if data is older than threshold
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - long.timestamp > TICKER_STALE_MS || now - short.timestamp > TICKER_STALE_MS {
                        return;
                    }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); // 0.1% buffer

                    let fee_long_total = fee_long * Decimal::from(2);
                    let fee_short_total = fee_short * Decimal::from(2);
                    let slippage_total = slippage * Decimal::from(2);

                    let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    let threshold = {
                        let cfg = self.config.lock().unwrap_or_else(|e| e.into_inner());
                        Decimal::try_from(cfg.min_spread_threshold).unwrap_or(Decimal::from(1))
                    };
                    let max_sanity = Decimal::from(50); // 50% max
                    
                    if net_spread >= threshold && net_spread < max_sanity {
                        // Prepare Risk Data
                        let _target_volume = Decimal::from(1000); // 1000 USDT target
                        
                        let mut depth_map = HashMap::new();
                        depth_map.insert(long.exchange, crate::model::OrderBookDepth { bids: long.bids.clone(), asks: long.asks.clone() });
                        depth_map.insert(short.exchange, crate::model::OrderBookDepth { bids: short.bids.clone(), asks: short.asks.clone() });

                        let opp_for_risk = ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            long_exchange: long.exchange,
                            short_exchange: short.exchange,
                            long_price: b_long.0,
                            short_price: b_short.0,
                            spread_pct: net_spread,
                            volume_usdt: Decimal::ZERO, // set when sending to execution
                            _timestamp: chrono::Utc::now().timestamp_millis(),
                        };

                        // Dynamic position size: cap by order book depth and equity fraction
                        let default_volume = target_volume_usdt();
                        let safe_volume = self.risk_manager.max_safe_volume_usdt(
                            &opp_for_risk,
                            &depth_map,
                            max_slippage_for_safe_volume(),
                        ).unwrap_or(default_volume);
                        let total_equity = {
                            let acct = self.account_state.lock().unwrap_or_else(|e| e.into_inner());
                            acct.total_equity_usdt
                        };
                        let max_pos_pct = max_position_percentage();
                        let equity_cap = if total_equity > Decimal::ZERO && max_pos_pct > Decimal::ZERO {
                            total_equity * max_pos_pct
                        } else {
                            default_volume
                        };
                        let target_volume = safe_volume.min(default_volume).min(equity_cap);

                        // Require expected profit (after spread) to exceed transfer/fee buffer
                        let expected_profit_usdt = target_volume * (net_spread / Decimal::from(100));
                        if expected_profit_usdt <= transfer_fee_usdt() {
                            return;
                        }

                        let r_rates = self.funding_rates.lock().unwrap_or_else(|e| e.into_inner());
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
                        {
                            let acct = self.account_state.lock().unwrap_or_else(|e| e.into_inner());
                            for eid in &[long.exchange, short.exchange] {
                                if let Some(exchange_statuses) = acct.asset_statuses.get(eid) {
                                    if let Some(usdt_status) = exchange_statuses.get("USDT") {
                                        status_map.insert(*eid, usdt_status.clone());
                                        continue;
                                    }
                                }
                                status_map.insert(*eid, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
                            }
                        }

                        let r_filters = self.market_filters.lock().unwrap_or_else(|e| e.into_inner());
                        
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
                            volume_usdt: target_volume,
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
                                    volume_usdt: target_volume,
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



    fn current_opportunities(&self, limit: usize) -> Vec<(String, ExchangeId, Decimal, ExchangeId, Decimal, Decimal, Decimal)> {
        let mut opportunities = Vec::new();

        for (symbol, exchanges) in &self.market_data {
            if exchanges.len() < 2 { continue; }

            {
                let filters = self.market_filters.lock().unwrap_or_else(|e| e.into_inner());
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

                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from_str("1.1").unwrap() { continue; }

                    // Freshness check for Matrix TUI
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - long.timestamp > TICKER_STALE_MS || now - short.timestamp > TICKER_STALE_MS { continue; }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long = long.exchange.taker_fee();
                    let fee_short = short.exchange.taker_fee();
                    let slippage = Decimal::new(1, 3); // 0.1% buffer
                    
                    let fee_long_total = fee_long * Decimal::from(2);
                    let fee_short_total = fee_short * Decimal::from(2);
                    let slippage_total = slippage * Decimal::from(2);
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
        if opportunities.len() > limit {
            opportunities.truncate(limit);
        }
        opportunities
    }

    fn print_arbitrage_matrix(&self) {
        use crossterm::{execute, terminal::{Clear, ClearType}, cursor::{MoveTo, Hide}};
        use std::io::stdout;
        use prettytable::{Table, Row, Cell, format};

        let mut stdout = stdout();
        
        let _ = execute!(stdout, Hide, Clear(ClearType::All), MoveTo(0, 0));

        let threshold = self.config.lock().unwrap_or_else(|e| e.into_inner()).min_spread_threshold;
        println!("=== ARBITRAGE MATRIX (Threshold: {:.1}%+) ===", threshold);
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

        let opportunities = self.current_opportunities(5);

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
            let s = self.account_state.lock().unwrap_or_else(|e| e.into_inner());
            s.clone()
        };

        let advices = self.rebalance_advisor.check(&state);
        
        {
            let mut last = self.last_rebalance_advice.lock().unwrap_or_else(|e| e.into_inner());
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

    async fn broadcast_tickers(&self) {
        // Only publish tickers for symbols present on 2+ exchanges — these are
        // the only ones relevant for arbitrage. Keeps payload small (~30-50 KB).
        use std::collections::HashMap as HM;
        let multi_exchange_symbols: Vec<&String> = self.market_data
            .iter()
            .filter(|(_, per_ex)| per_ex.len() >= 2)
            .map(|(sym, _)| sym)
            .collect();

        if multi_exchange_symbols.is_empty() { return; }

        let mut by_exchange: HM<String, Vec<&crate::model::UnifiedTicker>> = HM::new();
        for sym in &multi_exchange_symbols {
            if let Some(per_ex) = self.market_data.get(*sym) {
                for ticker in per_ex.values() {
                    by_exchange.entry(format!("{:?}", ticker.exchange)).or_default().push(ticker);
                }
            }
        }
        for (exchange, tickers) in &by_exchange {
            self.broadcast_message(&format!("ticker.{}", exchange), tickers).await;
        }
    }

    async fn broadcast_state(&self) {
        let state = self.account_state.lock().unwrap().clone();
        self.broadcast_message("account.state", &state).await;
    }

    async fn broadcast_config(&self) {
        let config_snapshot = { self.config.lock().unwrap_or_else(|e| e.into_inner()).clone() };
        self.broadcast_message("bot.config", &config_snapshot).await;
    }

    async fn broadcast_message<T: serde::Serialize>(&self, routing_key: &str, payload: &T) {
        let mut msg = self.messaging.lock().await;
        if let Some(client) = msg.as_mut() {
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
                crate::model::BotCommand::UpdateConfig { config: new_conf } => {
                    let mut conf = self.config.lock().unwrap();
                    *conf = new_conf;
                    let _ = conf.save();
                    info!("CONFIG_UPDATED: New thresholds applied.");
                }
                crate::model::BotCommand::UpdateSecrets { secrets: new_secrets } => {
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
                _ => {
                    // Handle other command types if needed
                    info!("Command type not handled in consumer: {:?}", cmd);
                }
            }
        }
        let _ = channel.basic_ack(BasicAckArguments::new(deliver.delivery_tag(), false)).await;
    }
}
