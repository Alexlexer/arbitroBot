use crate::config::AppConfig;
use crate::model::{ArbitrageOpportunity, ExchangeId};
use crate::rate_limiter::RateLimiter;
use hmac::{Hmac, Mac};
use log::{error, info};
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
}

impl ExecutionActor {
    pub fn new(
        rx: Receiver<ArbitrageOpportunity>,
        rate_limiter: Arc<RateLimiter>,
        config: Arc<std::sync::Mutex<AppConfig>>,
    ) -> Self {
        Self { rx, rate_limiter, config }
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

    async fn execute_opportunity(&self, opp: ArbitrageOpportunity, live: bool) {
        info!(
            "👀 OPPORTUNITY: Long {} on {} @ {}, Short {} on {} @ {} | Spread: {:.2}% | Vol: {} USDT",
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

        if live {
            let long_res = self
                .place_order_live(&opp, opp.long_exchange, "BUY", opp.long_price, opp.volume_usdt)
                .await;
            let short_res = self
                .place_order_live(&opp, opp.short_exchange, "SELL", opp.short_price, opp.volume_usdt)
                .await;
            if let Err(e) = long_res {
                error!("Long order failed: {}", e);
            }
            if let Err(e) = short_res {
                error!("Short order failed: {}", e);
            }
        } else {
            sleep(Duration::from_millis(50)).await;
            info!("   -> [SIM] Long {} on {:?} @ {}", opp.symbol, opp.long_exchange, opp.long_price);
            info!("   -> [SIM] Short {} on {:?} @ {}", opp.symbol, opp.short_exchange, opp.short_price);
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
        let _ = (opp, side, price, volume_usdt);
        let creds = match self.get_credentials(exchange) {
            Some(c) => c,
            None => return Err(format!("No API credentials for {:?}", exchange)),
        };

        match exchange {
            ExchangeId::Binance => self.place_binance_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bybit => self.place_bybit_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            _ => Err(format!("Live execution not implemented for {:?}", exchange)),
        }
    }

    fn hmac_sign(secret: &str, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
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
}
