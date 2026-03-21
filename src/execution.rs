use crate::config::AppConfig;
use crate::model::{ArbitrageOpportunity, ExchangeId, TradeRecord, TradeStatus};
use crate::rate_limiter::RateLimiter;
use hmac::{Hmac, Mac};
use log::{error, info, warn};
use rust_decimal::Decimal;
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::time::{sleep, Duration};

type HmacSha256 = Hmac<Sha256>;

pub struct ExecutionActor {
    rx: Receiver<ArbitrageOpportunity>,
    rate_limiter: Arc<RateLimiter>,
    config: Arc<std::sync::Mutex<AppConfig>>,
    trade_log: Vec<TradeRecord>,
}

impl ExecutionActor {
    pub fn new(
        rx: Receiver<ArbitrageOpportunity>,
        rate_limiter: Arc<RateLimiter>,
        config: Arc<std::sync::Mutex<AppConfig>>,
    ) -> Self {
        Self { rx, rate_limiter, config, trade_log: Vec::new() }
    }

    pub async fn run(&mut self) {
        info!("Execution Actor started.");
        let live = std::env::var("ENABLE_LIVE_TRADING").map(|v| v == "true" || v == "1").unwrap_or(false);
        if live {
            info!("ENABLE_LIVE_TRADING=true: real orders will be sent to exchanges.");
        } else {
            info!("ENABLE_LIVE_TRADING not set: execution is simulated (no real orders).");
        }

        while let Some(opp) = self.rx.recv().await {
            self.execute_opportunity(opp, live).await;
        }
    }

    async fn execute_opportunity(&mut self, opp: ArbitrageOpportunity, live: bool) {
        info!(
            "OPPORTUNITY: Long {} on {} @ {}, Short {} on {} @ {} | Spread: {:.2}% | Vol: {} USDT",
            opp.symbol, opp.long_exchange, opp.long_price, opp.symbol, opp.short_exchange, opp.short_price,
            opp.spread_pct, opp.volume_usdt
        );

        if !self.rate_limiter.check_limit(opp.long_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Order Rate Limit Exceeded", opp.long_exchange);
            return;
        }
        if !self.rate_limiter.check_limit(opp.short_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Order Rate Limit Exceeded", opp.short_exchange);
            return;
        }

        let trade_id = format!("{}_{}_{}", opp.symbol, opp.long_exchange, chrono::Utc::now().timestamp_millis());

        if live {
            let (long_res, short_res) = tokio::join!(
                self.place_order_live(&opp, opp.long_exchange, "BUY", opp.long_price, opp.volume_usdt),
                self.place_order_live(&opp, opp.short_exchange, "SELL", opp.short_price, opp.volume_usdt),
            );

            let long_status = match &long_res {
                Ok(_) => TradeStatus::Filled,
                Err(e) => TradeStatus::Failed(e.clone()),
            };
            let short_status = match &short_res {
                Ok(_) => TradeStatus::Filled,
                Err(e) => TradeStatus::Failed(e.clone()),
            };

            // Post-execution verification: if one leg failed, attempt reversal
            if long_res.is_ok() && short_res.is_err() {
                warn!("ROLLBACK: Short leg failed, reversing long position on {}", opp.long_exchange);
                let rev = self.place_order_live(&opp, opp.long_exchange, "SELL", opp.long_price, opp.volume_usdt).await;
                if let Err(e) = rev {
                    error!("ROLLBACK FAILED on {}: {} — MANUAL INTERVENTION REQUIRED", opp.long_exchange, e);
                } else {
                    info!("ROLLBACK: Long leg reversed on {}", opp.long_exchange);
                }
            } else if long_res.is_err() && short_res.is_ok() {
                warn!("ROLLBACK: Long leg failed, reversing short position on {}", opp.short_exchange);
                let rev = self.place_order_live(&opp, opp.short_exchange, "BUY", opp.short_price, opp.volume_usdt).await;
                if let Err(e) = rev {
                    error!("ROLLBACK FAILED on {}: {} — MANUAL INTERVENTION REQUIRED", opp.short_exchange, e);
                } else {
                    info!("ROLLBACK: Short leg reversed on {}", opp.short_exchange);
                }
            }

            // PnL tracking
            let pnl = if long_res.is_ok() && short_res.is_ok() {
                let notional = opp.volume_usdt;
                Some(notional * opp.spread_pct / Decimal::from(100))
            } else {
                None
            };

            let record = TradeRecord {
                id: trade_id.clone(),
                symbol: opp.symbol.clone(),
                long_exchange: opp.long_exchange,
                short_exchange: opp.short_exchange,
                long_price: opp.long_price,
                short_price: opp.short_price,
                volume_usdt: opp.volume_usdt,
                spread_pct: opp.spread_pct,
                long_status,
                short_status,
                realized_pnl_usdt: pnl,
                timestamp: chrono::Utc::now().timestamp_millis(),
            };

            if let Some(p) = pnl {
                info!("TRADE PnL [{}]: estimated ${:.4} USDT", trade_id, p);
            }

            self.trade_log.push(record);
            if self.trade_log.len() > 1000 {
                self.trade_log.drain(0..500);
            }

            if let Err(e) = &long_res { error!("Long order failed: {}", e); }
            if let Err(e) = &short_res { error!("Short order failed: {}", e); }
        } else {
            sleep(Duration::from_millis(50)).await;
            info!("   -> [SIM] Long {} on {:?} @ {}", opp.symbol, opp.long_exchange, opp.long_price);
            info!("   -> [SIM] Short {} on {:?} @ {}", opp.symbol, opp.short_exchange, opp.short_price);

            let pnl = opp.volume_usdt * opp.spread_pct / Decimal::from(100);
            let record = TradeRecord {
                id: trade_id,
                symbol: opp.symbol.clone(),
                long_exchange: opp.long_exchange,
                short_exchange: opp.short_exchange,
                long_price: opp.long_price,
                short_price: opp.short_price,
                volume_usdt: opp.volume_usdt,
                spread_pct: opp.spread_pct,
                long_status: TradeStatus::Filled,
                short_status: TradeStatus::Filled,
                realized_pnl_usdt: Some(pnl),
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            self.trade_log.push(record);
            if self.trade_log.len() > 1000 {
                self.trade_log.drain(0..500);
            }
        }
    }

    fn get_credentials(&self, exchange: ExchangeId) -> Option<(String, String, String)> {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        let creds = config.api_keys.get(&exchange).map(|c| (c.key.clone(), c.secret.clone(), c.passphrase.clone().unwrap_or_default()));
        if let Some((k, s, p)) = creds {
            return Some((k, s, p));
        }
        match exchange {
            ExchangeId::Binance => {
                let k = std::env::var("BINANCE_API_KEY").ok()?;
                let s = std::env::var("BINANCE_API_SECRET").ok()?;
                Some((k, s, String::new()))
            }
            ExchangeId::Bybit => {
                let k = std::env::var("BYBIT_API_KEY").ok()?;
                let s = std::env::var("BYBIT_API_SECRET").ok()?;
                Some((k, s, String::new()))
            }
            ExchangeId::Bitget => {
                let k = std::env::var("BITGET_API_KEY").ok()?;
                let s = std::env::var("BITGET_API_SECRET").ok()?;
                let p = std::env::var("BITGET_API_PASSPHRASE").unwrap_or_default();
                Some((k, s, p))
            }
            ExchangeId::MEXC => {
                let k = std::env::var("MEXC_API_KEY").ok()?;
                let s = std::env::var("MEXC_API_SECRET").ok()?;
                Some((k, s, String::new()))
            }
            ExchangeId::Okx => {
                let k = std::env::var("OKX_API_KEY").ok()?;
                let s = std::env::var("OKX_API_SECRET").ok()?;
                let p = std::env::var("OKX_API_PASSPHRASE").unwrap_or_default();
                Some((k, s, p))
            }
            _ => None,
        }
    }

    async fn place_order_live(
        &self,
        opp: &ArbitrageOpportunity,
        exchange: ExchangeId,
        side: &str,
        price: Decimal,
        volume_usdt: Decimal,
    ) -> Result<(), String> {
        let creds = match self.get_credentials(exchange) {
            Some(c) => c,
            None => return Err(format!("No API credentials for {:?}", exchange)),
        };

        match exchange {
            ExchangeId::Binance => self.place_binance_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bybit => self.place_bybit_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bitget => self.place_bitget_futures(&creds.0, &creds.1, &creds.2, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::MEXC => self.place_mexc_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            _ => Err(format!("Live execution not implemented for {:?}", exchange)),
        }
    }

    fn hmac_sign(secret: &str, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn hmac_sign_base64(secret: &str, payload: &str) -> String {
        use base64::{Engine as _, engine::general_purpose};
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key size");
        mac.update(payload.as_bytes());
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    /// Binance USDT-M Futures: POST /fapi/v1/order (MARKET)
    async fn place_binance_futures(
        &self,
        key: &str,
        secret: &str,
        symbol: &str,
        side: &str,
        volume_usdt: Decimal,
        price: Decimal,
    ) -> Result<(), String> {
        let symbol_api = format!("{}USDT", symbol);
        let quantity = (volume_usdt / price).round_dp(6);
        if quantity.is_zero() {
            return Err("Quantity rounds to zero".to_string());
        }
        let qty_str = quantity.to_string();

        let client = reqwest::Client::new();
        let timestamp = chrono::Utc::now().timestamp_millis();
        let params = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
            symbol_api, side, qty_str, timestamp
        );
        let signature = Self::hmac_sign(secret, &params);
        let body = format!("{}&signature={}", params, signature);

        let resp = client
            .post("https://fapi.binance.com/fapi/v1/order")
            .header("X-MBX-APIKEY", key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Binance order failed {}: {}", status, text));
        }
        info!("   -> Binance Futures order sent: {} {} qty={}", side, symbol_api, qty_str);
        Ok(())
    }

    /// Bybit V5 USDT perpetual: POST /v5/order/create (Market order)
    async fn place_bybit_futures(
        &self,
        key: &str,
        secret: &str,
        symbol: &str,
        side: &str,
        volume_usdt: Decimal,
        price: Decimal,
    ) -> Result<(), String> {
        let symbol_api = format!("{}USDT", symbol);
        let quantity = (volume_usdt / price).round_dp(6);
        if quantity.is_zero() {
            return Err("Quantity rounds to zero".to_string());
        }
        let qty_str = quantity.to_string();

        let client = reqwest::Client::new();
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        let side_cap = if side == "BUY" { "Buy" } else { "Sell" };
        let body_str = format!(
            r#"{{"category":"linear","symbol":"{}","side":"{}","orderType":"Market","qty":"{}"}}"#,
            symbol_api, side_cap, qty_str
        );
        let sign_payload = format!("{}{}{}{}", timestamp, key, recv_window, body_str);
        let signature = Self::hmac_sign(secret, &sign_payload);

        let resp = client
            .post("https://api.bybit.com/v5/order/create")
            .header("X-BAPI-API-KEY", key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("Content-Type", "application/json")
            .body(body_str.clone())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Bybit order failed {}: {}", status, text));
        }
        info!("   -> Bybit order sent: {} {} qty={}", side, symbol_api, qty_str);
        Ok(())
    }

    /// Bitget V2 USDT Futures: POST /api/v2/mix/order/place-order
    async fn place_bitget_futures(
        &self,
        key: &str,
        secret: &str,
        passphrase: &str,
        symbol: &str,
        side: &str,
        volume_usdt: Decimal,
        price: Decimal,
    ) -> Result<(), String> {
        let symbol_api = format!("{}USDT", symbol);
        let quantity = (volume_usdt / price).round_dp(6);
        if quantity.is_zero() {
            return Err("Quantity rounds to zero".to_string());
        }
        let qty_str = quantity.to_string();

        let bg_side = if side == "BUY" { "buy" } else { "sell" };
        let trade_side = if side == "BUY" { "open" } else { "open" };

        let body_json = serde_json::json!({
            "symbol": symbol_api,
            "productType": "USDT-FUTURES",
            "marginMode": "crossed",
            "marginCoin": "USDT",
            "size": qty_str,
            "side": bg_side,
            "tradeSide": trade_side,
            "orderType": "market"
        });
        let body_str = body_json.to_string();

        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "POST";
        let request_path = "/api/v2/mix/order/place-order";
        let sign_payload = format!("{}{}{}{}", timestamp, method, request_path, body_str);
        let signature = Self::hmac_sign_base64(secret, &sign_payload);

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("https://api.bitget.com{}", request_path))
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Bitget order failed {}: {}", status, text));
        }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("code").and_then(|c| c.as_str()) != Some("00000") {
            return Err(format!("Bitget order rejected: {}", text));
        }
        info!("   -> Bitget order sent: {} {} qty={}", side, symbol_api, qty_str);
        Ok(())
    }

    /// MEXC Contract Futures: POST /api/v1/private/order/submit
    async fn place_mexc_futures(
        &self,
        key: &str,
        secret: &str,
        symbol: &str,
        side: &str,
        volume_usdt: Decimal,
        price: Decimal,
    ) -> Result<(), String> {
        let symbol_api = format!("{}_USDT", symbol);
        let quantity = (volume_usdt / price).round_dp(6);
        if quantity.is_zero() {
            return Err("Quantity rounds to zero".to_string());
        }

        // MEXC side: 1=open_long, 2=close_short, 3=open_short, 4=close_long
        let mexc_side = if side == "BUY" { 1 } else { 3 };

        let body_json = serde_json::json!({
            "symbol": symbol_api,
            "price": price.to_string(),
            "vol": quantity.to_string(),
            "leverage": 1,
            "side": mexc_side,
            "type": 5,        // market order (type 5 = market)
            "openType": 2,    // cross margin
        });
        let body_str = body_json.to_string();

        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let sign_str = format!("{}{}{}", key, &timestamp, "");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key size");
        mac.update(sign_str.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let client = reqwest::Client::new();
        let resp = client
            .post("https://contract.mexc.com/api/v1/private/order/submit")
            .header("ApiKey", key)
            .header("Request-Time", &timestamp)
            .header("Signature", &signature)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("MEXC order failed {}: {}", status, text));
        }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(format!("MEXC order rejected: {}", text));
        }
        info!("   -> MEXC order sent: {} {} qty={}", side, symbol_api, quantity);
        Ok(())
    }
}
