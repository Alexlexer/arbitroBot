use crate::constants::{
    TICKER_STALE_MS, OPPORTUNITY_ALERT_COOLDOWN_MS,
    target_volume_usdt, transfer_fee_usdt, max_slippage_for_safe_volume,
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
    messaging: crate::messaging::SharedMessaging,
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

    pub async fn run(&mut self) {
        info!("Aggregator started.");
        
        // We can use a tick interval to print the matrix periodically
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
                    self.broadcast_state().await;
                    self.broadcast_config().await;
                }
            }
        }
    }

    async fn handle_listener_alert(&mut self, alert: ListenerAlert) {
        // Always broadcast raw alert so UI can show it immediately.
        self.broadcast_message("listener.alert", &alert).await;

        // Only handle futures listings for now (based on Listener "kind").
        if alert.kind.to_lowercase() != "futures" {
            return;
        }

        let symbol_norm = crate::model::normalize_symbol(&alert.symbol);
        let Some(exchanges) = self.market_data.get(&symbol_norm) else {
            return;
        };
        if exchanges.len() < 2 {
            return;
        }

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

        // Fee model matches detect_sharps()/current_opportunities().
        let fee_long_total = best_long.exchange.taker_fee() * Decimal::from(2);
        let fee_short_total = best_short.exchange.taker_fee() * Decimal::from(2);
        let slippage_total = Decimal::new(2, 3); // 0.2%
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
                BotCommand::DashboardLogin { .. } | BotCommand::DashboardRegister { .. } => {}
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
        
        // Check for immediate opportunity on every update (High-Frequency style)
        self.detect_sharps(&symbol).await;
        
        // Broadcast ticker update
        self.broadcast_message(&format!("ticker.{}", ticker.exchange), &ticker).await;
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
                remaining = Decimal::ZERO;
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
        if let Some(exchanges) = self.market_data.get(symbol) {
            if exchanges.len() < 2 { return; }

            let best_long = exchanges.values()
                .min_by(|a, b| a.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX).partial_cmp(&b.best_ask().map(|x| x.0).unwrap_or(Decimal::MAX)).unwrap_or(std::cmp::Ordering::Equal));
            
            let best_short = exchanges.values()
                .max_by(|a, b| a.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN).partial_cmp(&b.best_bid().map(|x| x.0).unwrap_or(Decimal::MIN)).unwrap_or(std::cmp::Ordering::Equal));

            if let (Some(long), Some(short)) = (best_long, best_short) {
                if long.exchange == short.exchange { return; }

                if let (Some(b_long), Some(b_short)) = (long.best_ask(), short.best_bid()) {
                    let floor = Decimal::from_str("0.00000001").unwrap();
                    if b_long.0 <= floor || b_short.0 <= floor { return; }

                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from(11) { 
                        info!("REJECT {}: Price ratio too high ({:.2}x)", symbol, price_ratio);
                        return; 
                    }

                    // Freshness Check: ignore if data is older than threshold
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - long.timestamp > TICKER_STALE_MS || now - short.timestamp > TICKER_STALE_MS {
                        info!("REJECT {}: Stale data (L: {}ms, S: {}ms ago)", symbol, now - long.timestamp, now - short.timestamp);
                        return;
                    }

                    let gross_spread = (b_short.0 - b_long.0) / b_long.0 * Decimal::from(100);
                    
                    let fee_long_total = long.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let fee_short_total = short.exchange.taker_fee() * Decimal::from(2); // Open + Close
                    let slippage_total = Decimal::new(2, 3); // 0.2% total buffer (0.1% entry + 0.1% exit)

                    let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
                    let net_spread = gross_spread - total_cost_pct;

                    // Optional VWAP-based pricing and diagnostics
                    let (depth_usdt, use_vwap_pricing, threshold, is_long_enabled, is_short_enabled) = {
                        let c = self.config.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            c.depth_usdt,
                            c.use_vwap_pricing,
                            c.min_spread_threshold,
                            c.enabled_exchanges.get(&long.exchange).cloned().unwrap_or(true),
                            c.enabled_exchanges.get(&short.exchange).cloned().unwrap_or(true)
                        )
                    };
                    let max_sanity = Decimal::from(100); // 100% max
                    let mut effective_long_price = b_long.0;
                    let mut effective_short_price = b_short.0;
                    let mut effective_net_spread = net_spread;

                    if depth_usdt > Decimal::ZERO {
                        let long_book = crate::model::OrderBookDepth { bids: long.bids.clone(), asks: long.asks.clone() };
                        let short_book = crate::model::OrderBookDepth { bids: short.bids.clone(), asks: short.asks.clone() };
                        if let (Some((vwap_long, filled_long_usdt)), Some((vwap_short, filled_short_usdt))) = (
                            self.vwap_for_volume_usdt(&long_book, "buy", depth_usdt),
                            self.vwap_for_volume_usdt(&short_book, "sell", depth_usdt),
                        ) {
                            let filled_ratio_long = (filled_long_usdt / depth_usdt).min(Decimal::from(1));
                            let filled_ratio_short = (filled_short_usdt / depth_usdt).min(Decimal::from(1));
                            let vwap_gross = (vwap_short - vwap_long) / vwap_long * Decimal::from(100);
                            let vwap_net = vwap_gross - total_cost_pct;
                            info!(
                                "VWAP DIAG {} {}→{} depth={} filled L={:.2} S={:.2} gross={:.2}% net={:.2}% (topbook net={:.2}%)",
                                symbol,
                                long.exchange,
                                short.exchange,
                                depth_usdt,
                                filled_ratio_long * Decimal::from(100),
                                filled_ratio_short * Decimal::from(100),
                                vwap_gross,
                                vwap_net,
                                net_spread,
                            );

                            // Optionally switch pricing to VWAP if feature flag is enabled
                            // and both sides have filled most of the requested depth.
                            let min_fill_ratio = Decimal::new(8, 1); // 0.8
                            if use_vwap_pricing
                                && filled_ratio_long >= min_fill_ratio
                                && filled_ratio_short >= min_fill_ratio
                            {
                                effective_long_price = vwap_long;
                                effective_short_price = vwap_short;
                                effective_net_spread = vwap_net;
                                info!(
                                    "VWAP ACTIVE {} {}→{} using VWAP prices (L={:.6}, S={:.6}) net={:.2}%",
                                    symbol,
                                    long.exchange,
                                    short.exchange,
                                    effective_long_price,
                                    effective_short_price,
                                    effective_net_spread,
                                );
                            }
                        }
                    }
                    
                    if effective_net_spread >= threshold && effective_net_spread < max_sanity {
                        // Check if exchanges are enabled
                        if !is_long_enabled || !is_short_enabled {
                            return;
                        }

                        let mut depth_map = HashMap::new();
                        depth_map.insert(long.exchange, crate::model::OrderBookDepth { bids: long.bids.clone(), asks: long.asks.clone() });
                        depth_map.insert(short.exchange, crate::model::OrderBookDepth { bids: short.bids.clone(), asks: short.asks.clone() });

                        let opp_for_risk = ArbitrageOpportunity {
                            symbol: symbol.to_string(),
                            long_exchange: long.exchange,
                            short_exchange: short.exchange,
                            long_price: effective_long_price,
                            short_price: effective_short_price,
                            spread_pct: effective_net_spread,
                            volume_usdt: Decimal::ZERO, // set when sending to execution
                            _timestamp: chrono::Utc::now().timestamp_millis(),
                        };

                        // Dynamic position size: cap by order book depth (slippage budget)
                        let default_volume = target_volume_usdt();
                        let safe_volume = self.risk_manager.max_safe_volume_usdt(
                            &opp_for_risk,
                            &depth_map,
                            max_slippage_for_safe_volume(),
                        ).unwrap_or(default_volume);
                        let target_volume = safe_volume.min(default_volume);

                        // Require expected profit (after spread) to exceed transfer/fee buffer
                        let expected_profit_usdt = target_volume * (effective_net_spread / Decimal::from(100));
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
                        status_map.insert(long.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
                        status_map.insert(short.exchange, AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });

                        let r_filters = self.market_filters.lock().unwrap_or_else(|e| e.into_inner());
                        
                        let mut ticker_timestamps = HashMap::new();
                        ticker_timestamps.insert(long.exchange, long.timestamp);
                        ticker_timestamps.insert(short.exchange, short.timestamp);

                        let account_state = self.account_state.lock().unwrap_or_else(|e| e.into_inner());

                        // Validate Risk
                        let risk_result = self.risk_manager.validate(&opp_for_risk, &depth_map, &funding_map, &status_map, &r_filters, &ticker_timestamps, &account_state, target_volume).await;

                        // Cooldown: don't spam the same symbol+pair (e.g. 1 alert per minute per opportunity)
                        let alert_key = format!("{}_{:?}_{:?}", symbol, long.exchange, short.exchange);
                        let should_send = {
                            let mut last = self.last_opportunity_alert.lock().unwrap_or_else(|e| e.into_inner());
                            let last_ts = last.get(&alert_key).copied().unwrap_or(0);
                            if now - last_ts >= OPPORTUNITY_ALERT_COOLDOWN_MS {
                                last.insert(alert_key.clone(), now);
                                true
                            } else {
                                false
                            }
                        };

                        if should_send {
                            let status_text: String = match &risk_result {
                                Ok(_) => "✅ *ACTIONABLE*".into(),
                                Err(e) => format!("⚠️ *RISK BLOCKED*\n_Reason: {}_", e),
                            };
                            let alert_msg = format!(
                                "{} \n\n\
                                *Symbol*: {}\n\
                                *Long*: {} @ {:.4}\n\
                                *Short*: {} @ {:.4}\n\
                                *Net Spread*: {:.2}%\n\
                                *Threshold*: {:.1}%",
                                &status_text, symbol, long.exchange, b_long.0, short.exchange, b_short.0, net_spread, threshold
                            );
                            let n = self.notifier.clone();
                            tokio::spawn(async move {
                                n.send_alert(&alert_msg).await;
                            });
                        }

                        if let Ok(_) = risk_result {
                            let mut opp = opp_for_risk.clone();
                            opp.volume_usdt = target_volume;
                            if let Err(e) = self.exec_tx.send(opp).await {
                                error!("Failed to send opportunity to ExecutionActor: {}", e);
                            }
                        } else if let Err(e) = risk_result {
                            info!("Risk Check Failed for {}: {}", symbol, e);
                        }
                    }
                }
            }
        }
    }

    /// Build current opportunities (same logic as matrix), sorted by net spread desc, limited.
    fn current_opportunities(&self, limit: usize) -> Vec<(String, ExchangeId, Decimal, ExchangeId, Decimal, Decimal, Decimal)> {
        let mut opportunities = Vec::new();

        for (symbol, exchanges) in &self.market_data {
            if exchanges.len() < 2 { continue; }

            // Trading Status Check
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
                    let floor = Decimal::from_str("0.00000001").unwrap();
                    if b_long.0 <= floor || b_short.0 <= floor { continue; }

                    // Magnitude Check: Filter out unit mismatches (1:1000 etc) or different coins
                    // Price ratio > 2.0x difference is almost always a unit or coin mismatch
                    let price_ratio = if b_long.0 > b_short.0 { b_long.0 / b_short.0 } else { b_short.0 / b_long.0 };
                    if price_ratio > Decimal::from_str("1.1").unwrap() { continue; }

                    // Freshness check for Matrix TUI
                    let now = chrono::Utc::now().timestamp_millis();
                    if now - long.timestamp > TICKER_STALE_MS || now - short.timestamp > TICKER_STALE_MS { continue; }

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

        for (sym, l_ex, l_p, s_ex, s_p, gross, net) in opportunities {
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
            let s = self.account_state.lock().unwrap_or_else(|e| e.into_inner());
            s.clone()
        };

        let advices = self.rebalance_advisor.check(&state);
        
        // Store for TUI
        {
            let mut last = self.last_rebalance_advice.lock().unwrap_or_else(|e| e.into_inner());
            *last = advices.clone();
        }

        // Debounce: only send Telegram when advice changed (avoids spam every 2s)
        let should_notify = {
            let last_sent = self.last_rebalance_notified.lock().unwrap_or_else(|e| e.into_inner());
            last_sent.as_ref() != Some(&advices)
        };
        if should_notify && !advices.is_empty() {
            if let Ok(mut last) = self.last_rebalance_notified.lock() {
                *last = Some(advices.clone());
            }
            for advice in &advices {
                let msg = format!(
                    "⚖️ *Rebalance Suggestion*\n\n\
                    *Reason*: {}\n\
                    *Action*: Move `${:.0} USDT` from **{}** to **{}**",
                    advice.reason, advice.amount_usdt, advice.from_exchange, advice.to_exchange
                );
                info!("REBALANCE: {}", advice.reason);
                let n = self.notifier.clone();
                tokio::spawn(async move {
                    n.send_alert(&msg).await;
                });
            }
        }
    }

    async fn broadcast_state(&self) {
        let state = self.account_state.lock().unwrap_or_else(|e| e.into_inner()).clone();
        self.broadcast_message("account.state", &state).await;
    }

    async fn broadcast_config(&self) {
        let config_snapshot = { self.config.lock().unwrap_or_else(|e| e.into_inner()).clone() };
        self.broadcast_message("bot.config", &config_snapshot).await;
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
