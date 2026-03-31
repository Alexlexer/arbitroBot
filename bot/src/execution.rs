use crate::config::AppConfig;
use crate::model::{ArbitrageOpportunity, ExchangeId, TradeRecord, TradeStatus};
use crate::rate_limiter::RateLimiter;
use hmac::{Hmac, Mac};
use sha2::{Sha256, Sha512, Digest};
use sha3::Keccak256;
use log::{error, info, warn};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::time::{sleep, Duration};

// ── Hyperliquid msgpack order structs ─────────────────────────────────────────
#[derive(serde::Serialize)]
struct HlOrderAction {
    #[serde(rename = "type")]
    type_: &'static str,
    orders: Vec<HlOrder>,
    grouping: &'static str,
}

#[derive(serde::Serialize)]
struct HlOrder {
    a: u32,
    b: bool,
    p: String,
    s: String,
    r: bool,
    t: HlOrderTif,
}

#[derive(serde::Serialize)]
struct HlOrderTif {
    limit: HlLimitTif,
}

#[derive(serde::Serialize)]
struct HlLimitTif {
    tif: String,
}

type HmacSha256 = Hmac<Sha256>;
type HmacSha512 = Hmac<Sha512>;

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
        info!("Execution Actor started (live trading controlled by config).");
        while let Some(opp) = self.rx.recv().await {
            let live = self.config.lock().unwrap_or_else(|e| e.into_inner()).live_trading_enabled;
            self.execute_opportunity(opp, live).await;
        }
    }

    async fn execute_opportunity(&mut self, opp: ArbitrageOpportunity, live: bool) {
        info!(
            "OPPORTUNITY: Long {} on {} @ {}, Short {} on {} @ {} | Spread: {:.2}% | Vol: {} USDT",
            opp.symbol, opp.long_exchange, opp.long_price,
            opp.symbol, opp.short_exchange, opp.short_price,
            opp.spread_pct, opp.volume_usdt
        );

        if !self.rate_limiter.check_limit(opp.long_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Rate Limit", opp.long_exchange);
            return;
        }
        if !self.rate_limiter.check_limit(opp.short_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Rate Limit", opp.short_exchange);
            return;
        }

        let trade_id = format!("{}_{}_{}", opp.symbol, opp.long_exchange, chrono::Utc::now().timestamp_millis());

        if live {
            let (long_res, short_res) = tokio::join!(
                self.place_order_live(&opp, opp.long_exchange, "BUY",  opp.long_price,  opp.volume_usdt),
                self.place_order_live(&opp, opp.short_exchange, "SELL", opp.short_price, opp.volume_usdt),
            );

            // Rollback on partial fill
            if long_res.is_ok() && short_res.is_err() {
                warn!("ROLLBACK: Short failed on {}, reversing long on {}", opp.short_exchange, opp.long_exchange);
                let rev = self.place_order_live(&opp, opp.long_exchange, "SELL", opp.long_price, opp.volume_usdt).await;
                if let Err(e) = rev { error!("ROLLBACK FAILED on {}: {} — MANUAL INTERVENTION REQUIRED", opp.long_exchange, e); }
            } else if long_res.is_err() && short_res.is_ok() {
                warn!("ROLLBACK: Long failed on {}, reversing short on {}", opp.long_exchange, opp.short_exchange);
                let rev = self.place_order_live(&opp, opp.short_exchange, "BUY", opp.short_price, opp.volume_usdt).await;
                if let Err(e) = rev { error!("ROLLBACK FAILED on {}: {} — MANUAL INTERVENTION REQUIRED", opp.short_exchange, e); }
            }

            let pnl = if long_res.is_ok() && short_res.is_ok() {
                Some(opp.volume_usdt * opp.spread_pct / Decimal::from(100))
            } else { None };

            let long_status  = match &long_res  { Ok(_) => TradeStatus::Filled, Err(e) => TradeStatus::Failed(e.clone()) };
            let short_status = match &short_res { Ok(_) => TradeStatus::Filled, Err(e) => TradeStatus::Failed(e.clone()) };

            if let Err(e) = &long_res  { error!("Long  order failed: {}", e); }
            if let Err(e) = &short_res { error!("Short order failed: {}", e); }
            if let Some(p) = pnl { info!("TRADE PnL [{}]: est ${:.4} USDT", trade_id, p); }

            self.push_record(TradeRecord {
                id: trade_id, symbol: opp.symbol,
                long_exchange: opp.long_exchange, short_exchange: opp.short_exchange,
                long_price: opp.long_price, short_price: opp.short_price,
                volume_usdt: opp.volume_usdt, spread_pct: opp.spread_pct,
                long_status, short_status, realized_pnl_usdt: pnl,
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        } else {
            sleep(Duration::from_millis(50)).await;
            info!("   -> [SIM] Long  {} on {:?} @ {}", opp.symbol, opp.long_exchange,  opp.long_price);
            info!("   -> [SIM] Short {} on {:?} @ {}", opp.symbol, opp.short_exchange, opp.short_price);
            let pnl = opp.volume_usdt * opp.spread_pct / Decimal::from(100);
            self.push_record(TradeRecord {
                id: trade_id, symbol: opp.symbol,
                long_exchange: opp.long_exchange, short_exchange: opp.short_exchange,
                long_price: opp.long_price, short_price: opp.short_price,
                volume_usdt: opp.volume_usdt, spread_pct: opp.spread_pct,
                long_status: TradeStatus::Filled, short_status: TradeStatus::Filled,
                realized_pnl_usdt: Some(pnl),
                timestamp: chrono::Utc::now().timestamp_millis(),
            });
        }
    }

    fn push_record(&mut self, r: TradeRecord) {
        self.trade_log.push(r);
        if self.trade_log.len() > 1000 { self.trade_log.drain(0..500); }
    }

    fn get_credentials(&self, exchange: ExchangeId) -> Option<(String, String, String)> {
        let config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = config.api_keys.get(&exchange) {
            return Some((c.key.clone(), c.secret.clone(), c.passphrase.clone().unwrap_or_default()));
        }
        // Env var fallback
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
            ExchangeId::Gate => {
                let k = std::env::var("GATE_API_KEY").ok()?;
                let s = std::env::var("GATE_API_SECRET").ok()?;
                Some((k, s, String::new()))
            }
            ExchangeId::Bitmart => {
                let k = std::env::var("BITMART_API_KEY").ok()?;
                let s = std::env::var("BITMART_API_SECRET").ok()?;
                let memo = std::env::var("BITMART_API_MEMO").unwrap_or_default();
                Some((k, s, memo))
            }
            ExchangeId::Kraken => {
                let k = std::env::var("KRAKEN_API_KEY").ok()?;
                let s = std::env::var("KRAKEN_API_SECRET").ok()?;
                Some((k, s, String::new()))
            }
            ExchangeId::Hyperliquid => {
                let pk = std::env::var("HYPERLIQUID_PRIVATE_KEY").ok()?;
                Some((pk.clone(), pk, String::new()))
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
        let creds = self.get_credentials(exchange)
            .ok_or_else(|| format!("No credentials for {:?}", exchange))?;

        match exchange {
            ExchangeId::Binance   => self.place_binance_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bybit     => self.place_bybit_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bitget    => self.place_bitget_futures(&creds.0, &creds.1, &creds.2, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::MEXC      => self.place_mexc_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Okx       => self.place_okx_futures(&creds.0, &creds.1, &creds.2, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Gate      => self.place_gate_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Bitmart   => self.place_bitmart_futures(&creds.0, &creds.1, &creds.2, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Kraken       => self.place_kraken_futures(&creds.0, &creds.1, &opp.symbol, side, volume_usdt, price).await,
            ExchangeId::Hyperliquid  => self.place_hyperliquid_perp(&creds.0, &opp.symbol, side, volume_usdt, price).await,
            other => Err(format!("Live execution not implemented for {:?}", other)),
        }
    }

    // ── Signing helpers ────────────────────────────────────────────────────────

    fn hmac_sha256_hex(secret: &str, payload: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn hmac_sha256_b64(secret: &str, payload: &str) -> String {
        use base64::{Engine as _, engine::general_purpose};
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC key");
        mac.update(payload.as_bytes());
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    fn hmac_sha512_hex(secret: &[u8], payload: &[u8]) -> String {
        let mut mac = HmacSha512::new_from_slice(secret).expect("HMAC key");
        mac.update(payload);
        hex::encode(mac.finalize().into_bytes())
    }

    fn hmac_sha512_b64(secret: &[u8], payload: &[u8]) -> String {
        use base64::{Engine as _, engine::general_purpose};
        let mut mac = HmacSha512::new_from_slice(secret).expect("HMAC key");
        mac.update(payload);
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    // ── Binance USDT-M Futures ─────────────────────────────────────────────────

    async fn place_binance_futures(
        &self, key: &str, secret: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let sym = format!("{}USDT", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        let ts = chrono::Utc::now().timestamp_millis();
        let params = format!("symbol={}&side={}&type=MARKET&quantity={}&timestamp={}", sym, side, qty, ts);
        let sig = Self::hmac_sha256_hex(secret, &params);

        let resp = reqwest::Client::new()
            .post("https://fapi.binance.com/fapi/v1/order")
            .header("X-MBX-APIKEY", key)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("{}&signature={}", params, sig))
            .send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Binance {}: {}", status, text)); }
        info!("   -> Binance Futures {} {} qty={}", side, sym, qty);
        Ok(())
    }

    // ── Bybit V5 USDT Perpetual ────────────────────────────────────────────────

    async fn place_bybit_futures(
        &self, key: &str, secret: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let sym = format!("{}USDT", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        let ts = chrono::Utc::now().timestamp_millis().to_string();
        let rw = "5000";
        let side_cap = if side == "BUY" { "Buy" } else { "Sell" };
        let body = format!(r#"{{"category":"linear","symbol":"{}","side":"{}","orderType":"Market","qty":"{}"}}"#, sym, side_cap, qty);
        let sig = Self::hmac_sha256_hex(secret, &format!("{}{}{}{}", ts, key, rw, body));

        let resp = reqwest::Client::new()
            .post("https://api.bybit.com/v5/order/create")
            .header("X-BAPI-API-KEY", key)
            .header("X-BAPI-TIMESTAMP", &ts)
            .header("X-BAPI-SIGN", &sig)
            .header("X-BAPI-RECV-WINDOW", rw)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Bybit {}: {}", status, text)); }
        info!("   -> Bybit Futures {} {} qty={}", side, sym, qty);
        Ok(())
    }

    // ── Bitget V2 USDT Futures ─────────────────────────────────────────────────

    async fn place_bitget_futures(
        &self, key: &str, secret: &str, passphrase: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let sym = format!("{}USDT", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        let bg_side = if side == "BUY" { "buy" } else { "sell" };
        let body = serde_json::json!({
            "symbol": sym, "productType": "USDT-FUTURES",
            "marginMode": "crossed", "marginCoin": "USDT",
            "size": qty.to_string(), "side": bg_side,
            "tradeSide": "open", "orderType": "market"
        }).to_string();

        let ts = chrono::Utc::now().timestamp_millis().to_string();
        let path = "/api/v2/mix/order/place-order";
        let sig = Self::hmac_sha256_b64(secret, &format!("{}POST{}{}", ts, path, body));

        let resp = reqwest::Client::new()
            .post(format!("https://api.bitget.com{}", path))
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", sig)
            .header("ACCESS-TIMESTAMP", &ts)
            .header("ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Bitget {}: {}", status, text)); }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("code").and_then(|c| c.as_str()) != Some("00000") {
            return Err(format!("Bitget rejected: {}", text));
        }
        info!("   -> Bitget Futures {} {} qty={}", side, sym, qty);
        Ok(())
    }

    // ── MEXC Contract Futures ──────────────────────────────────────────────────

    async fn place_mexc_futures(
        &self, key: &str, secret: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let sym = format!("{}_USDT", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        let mexc_side = if side == "BUY" { 1 } else { 3 }; // 1=open_long, 3=open_short
        let body = serde_json::json!({
            "symbol": sym, "price": price.to_string(),
            "vol": qty.to_string(), "leverage": 1,
            "side": mexc_side, "type": 5, "openType": 2
        }).to_string();

        let ts = chrono::Utc::now().timestamp_millis().to_string();
        let sign_str = format!("{}{}{}", key, &ts, body);
        let sig = Self::hmac_sha256_hex(secret, &sign_str);

        let resp = reqwest::Client::new()
            .post("https://contract.mexc.com/api/v1/private/order/submit")
            .header("ApiKey", key)
            .header("Request-Time", &ts)
            .header("Signature", &sig)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("MEXC {}: {}", status, text)); }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("success").and_then(|v| v.as_bool()) != Some(true) {
            return Err(format!("MEXC rejected: {}", text));
        }
        info!("   -> MEXC Futures {} {} qty={}", side, sym, qty);
        Ok(())
    }

    // ── OKX USDT-Margined Swap ─────────────────────────────────────────────────

    async fn place_okx_futures(
        &self, key: &str, secret: &str, passphrase: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let inst = format!("{}-USDT-SWAP", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        let okx_side = if side == "BUY" { "buy" } else { "sell" };
        let pos_side = if side == "BUY" { "long" } else { "short" };
        let body = serde_json::json!({
            "instId": inst, "tdMode": "cross",
            "side": okx_side, "posSide": pos_side,
            "ordType": "market", "sz": qty.to_string()
        }).to_string();

        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let path = "/api/v5/trade/order";
        let prehash = format!("{}POST{}{}", ts, path, body);
        let sig = Self::hmac_sha256_b64(secret, &prehash);

        let resp = reqwest::Client::new()
            .post(format!("https://www.okx.com{}", path))
            .header("OK-ACCESS-KEY", key)
            .header("OK-ACCESS-SIGN", sig)
            .header("OK-ACCESS-TIMESTAMP", &ts)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("OKX {}: {}", status, text)); }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("code").and_then(|c| c.as_str()) != Some("0") {
            return Err(format!("OKX rejected: {}", text));
        }
        info!("   -> OKX Swap {} {} qty={}", side, inst, qty);
        Ok(())
    }

    // ── Gate.io USDT Perpetual ─────────────────────────────────────────────────
    // Auth: HMAC-SHA512. Sign string: "METHOD\nPATH\nQUERY\nBODY_SHA512_HEX\nTIMESTAMP"

    async fn place_gate_futures(
        &self, key: &str, secret: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let contract = format!("{}_USDT", symbol);
        let qty_raw = (volume_usdt / price).round_dp(0);
        if qty_raw.is_zero() { return Err("qty zero".into()); }
        // Gate: positive size = long, negative size = short
        let size: i64 = if side == "BUY" {
            qty_raw.to_string().parse::<i64>().unwrap_or(1)
        } else {
            -(qty_raw.to_string().parse::<i64>().unwrap_or(1))
        };

        let body = serde_json::json!({
            "contract": contract,
            "size": size,
            "price": "0",
            "tif": "ioc",
            "reduce_only": false
        }).to_string();

        let ts = chrono::Utc::now().timestamp().to_string();
        let path = "/api/v4/futures/usdt/orders";
        let body_hash = hex::encode(Sha512::digest(body.as_bytes()));
        let sign_str = format!("POST\n{}\n\n{}\n{}", path, body_hash, ts);
        let sig = Self::hmac_sha512_hex(secret.as_bytes(), sign_str.as_bytes());

        let resp = reqwest::Client::new()
            .post(format!("https://api.gateio.ws{}", path))
            .header("KEY", key)
            .header("SIGN", sig)
            .header("Timestamp", &ts)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Gate {}: {}", status, text)); }
        info!("   -> Gate Futures {} {} size={}", side, contract, size);
        Ok(())
    }

    // ── Bitmart Contract Futures ───────────────────────────────────────────────
    // Auth: HMAC-SHA256. Sign: timestamp#memo#body

    async fn place_bitmart_futures(
        &self, key: &str, secret: &str, memo: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        let sym = format!("{}USDT", symbol);
        let qty = (volume_usdt / price).round_dp(6);
        if qty.is_zero() { return Err("qty zero".into()); }

        // Bitmart side: 1=buy_open_long, 4=sell_open_short
        let bm_side = if side == "BUY" { 1u8 } else { 4u8 };
        let body = serde_json::json!({
            "symbol": sym, "side": bm_side,
            "type": "market", "vol": qty.to_string(),
            "open_type": "cross", "leverage": "1"
        }).to_string();

        let ts = chrono::Utc::now().timestamp_millis().to_string();
        let sign_content = format!("{}#{}", ts, body);
        let sig = Self::hmac_sha256_hex(secret, &sign_content);

        let resp = reqwest::Client::new()
            .post("https://api-cloud-v2.bitmart.com/contract/private/submit-order")
            .header("X-BM-KEY", key)
            .header("X-BM-SIGN", sig)
            .header("X-BM-TIMESTAMP", &ts)
            .header("X-BM-BROKER-ID", memo) // memo is used as broker id (optional)
            .header("Content-Type", "application/json")
            .body(body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Bitmart {}: {}", status, text)); }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("code").and_then(|c| c.as_i64()) != Some(1000) {
            return Err(format!("Bitmart rejected: {}", text));
        }
        info!("   -> Bitmart Futures {} {} qty={}", side, sym, qty);
        Ok(())
    }

    // ── Kraken Derivatives (Perpetual Futures) ─────────────────────────────────
    // Auth: SHA-256 + HMAC-SHA512 with base64-decoded secret

    async fn place_kraken_futures(
        &self, key: &str, secret: &str,
        symbol: &str, side: &str, volume_usdt: Decimal, price: Decimal,
    ) -> Result<(), String> {
        use base64::{Engine as _, engine::general_purpose};

        // Kraken futures symbol: PF_XBTUSD style — map our symbol to their format
        let kraken_sym = Self::to_kraken_futures_symbol(symbol);
        let qty = (volume_usdt / price).round_dp(0);
        if qty.is_zero() { return Err("qty zero".into()); }

        let kraken_side = if side == "BUY" { "buy" } else { "sell" };
        let nonce = chrono::Utc::now().timestamp_millis().to_string();
        let post_body = format!(
            "orderType=mkt&symbol={}&side={}&size={}&nonce={}",
            kraken_sym, kraken_side, qty, nonce
        );
        let path = "/derivatives/api/v3/sendorder";

        // Signature: base64(HMAC-SHA512(SHA256(post_body + nonce + path), base64_decode(secret)))
        let msg = format!("{}{}{}", post_body, nonce, path);
        let sha256_hash = Sha256::digest(msg.as_bytes());
        let secret_bytes = general_purpose::STANDARD.decode(secret)
            .map_err(|e| format!("Kraken secret decode: {}", e))?;
        let sig = Self::hmac_sha512_b64(&secret_bytes, &sha256_hash);

        let resp = reqwest::Client::new()
            .post(format!("https://futures.kraken.com{}", path))
            .header("APIKey", key)
            .header("Authent", sig)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_body).send().await.map_err(|e| e.to_string())?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() { return Err(format!("Kraken {}: {}", status, text)); }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed.get("result").and_then(|r| r.as_str()) != Some("success") {
            return Err(format!("Kraken rejected: {}", text));
        }
        info!("   -> Kraken Futures {} {} qty={}", side, kraken_sym, qty);
        Ok(())
    }

    fn to_kraken_futures_symbol(sym: &str) -> String {
        // Map common symbols to Kraken perpetual format (PF_ prefix for flex perps)
        match sym {
            "BTC" | "XBT" => "PF_XBTUSD".to_string(),
            "ETH"  => "PF_ETHUSD".to_string(),
            "SOL"  => "PF_SOLUSD".to_string(),
            "XRP"  => "PF_XRPUSD".to_string(),
            "DOGE" => "PF_DOGEUSD".to_string(),
            "LTC"  => "PF_LTCUSD".to_string(),
            "BCH"  => "PF_BCHUSD".to_string(),
            "LINK" => "PF_LINKUSD".to_string(),
            "ADA"  => "PF_ADAUSD".to_string(),
            "DOT"  => "PF_DOTUSD".to_string(),
            other  => format!("PF_{}USD", other),
        }
    }

    // ── Hyperliquid USDT Perpetual ─────────────────────────────────────────────
    // Auth: EIP-712 signed with secp256k1 wallet private key.
    // Action hash = keccak256(msgpack(action) + nonce_8be + 0x00).
    // Final hash = EIP-712 over Agent{source="a", connectionId=action_hash}.

    async fn place_hyperliquid_perp(
        &self,
        private_key: &str,
        symbol: &str,
        side: &str,
        volume_usdt: Decimal,
        price: Decimal,
    ) -> Result<(), String> {
        use k256::ecdsa::SigningKey;

        let client = reqwest::Client::new();
        let coin = symbol
            .trim_end_matches("USDT")
            .trim_end_matches("-PERP")
            .to_uppercase();

        // 1. Fetch meta to get asset index and szDecimals
        let meta: serde_json::Value = client
            .post("https://api.hyperliquid.xyz/info")
            .json(&serde_json::json!({"type": "meta"}))
            .send().await.map_err(|e| format!("HL meta: {}", e))?
            .json().await.map_err(|e| format!("HL meta parse: {}", e))?;

        let universe = meta["universe"].as_array()
            .ok_or("HL: no universe in meta")?;

        let (asset_idx, sz_decimals) = universe.iter().enumerate()
            .find_map(|(i, v)| {
                if v["name"].as_str() == Some(coin.as_str()) {
                    let sz = v["szDecimals"].as_u64().unwrap_or(4) as usize;
                    Some((i as u32, sz))
                } else { None }
            })
            .ok_or_else(|| format!("HL: {} not in universe", coin))?;

        let is_buy = side == "BUY";

        // 2. Price with small slippage so IOC fills
        let adj_price = if is_buy {
            price * Decimal::new(1005, 3)
        } else {
            price * Decimal::new(995, 3)
        };
        let size = (volume_usdt / adj_price).round_dp(sz_decimals as u32);
        if size.is_zero() { return Err("HL: qty zero".into()); }

        let price_str = format!("{:.2}", adj_price);
        let size_str  = format!("{:.prec$}", size, prec = sz_decimals);

        // 3. Build msgpack-encodable action
        let action = HlOrderAction {
            type_: "order",
            orders: vec![HlOrder {
                a: asset_idx,
                b: is_buy,
                p: price_str,
                s: size_str,
                r: false,
                t: HlOrderTif { limit: HlLimitTif { tif: "Ioc".into() } },
            }],
            grouping: "na",
        };

        // 4. Compute connection_id = keccak256(msgpack(action) + nonce_8be + 0x00)
        let nonce = chrono::Utc::now().timestamp_millis() as u64;
        let mut hash_input = rmp_serde::to_vec_named(&action)
            .map_err(|e| format!("HL msgpack: {}", e))?;
        hash_input.extend_from_slice(&nonce.to_be_bytes());
        hash_input.push(0x00);

        use sha3::Digest as _;
        let connection_id: [u8; 32] = Keccak256::digest(&hash_input).into();

        // 5. EIP-712 domain separator
        // keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)")
        let domain_type_hash: [u8; 32] = Keccak256::digest(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        ).into();
        let name_hash:    [u8; 32] = Keccak256::digest(b"Exchange").into();
        let version_hash: [u8; 32] = Keccak256::digest(b"1").into();
        let mut chain_id = [0u8; 32];
        chain_id[30] = 0x05; // 1337 = 0x0539
        chain_id[31] = 0x39;

        let mut domain_enc = Vec::with_capacity(160);
        domain_enc.extend_from_slice(&domain_type_hash);
        domain_enc.extend_from_slice(&name_hash);
        domain_enc.extend_from_slice(&version_hash);
        domain_enc.extend_from_slice(&chain_id);
        domain_enc.extend_from_slice(&[0u8; 32]); // verifyingContract = 0x0
        let domain_sep: [u8; 32] = Keccak256::digest(&domain_enc).into();

        // 6. Agent struct hash
        // keccak256("Agent(string source,bytes32 connectionId)")
        let agent_type_hash: [u8; 32] = Keccak256::digest(
            b"Agent(string source,bytes32 connectionId)"
        ).into();
        let source_hash: [u8; 32] = Keccak256::digest(b"a").into(); // "a" = mainnet

        let mut struct_enc = Vec::with_capacity(96);
        struct_enc.extend_from_slice(&agent_type_hash);
        struct_enc.extend_from_slice(&source_hash);
        struct_enc.extend_from_slice(&connection_id);
        let struct_hash: [u8; 32] = Keccak256::digest(&struct_enc).into();

        // 7. Final EIP-712 signing hash
        let mut msg = Vec::with_capacity(66);
        msg.push(0x19u8);
        msg.push(0x01u8);
        msg.extend_from_slice(&domain_sep);
        msg.extend_from_slice(&struct_hash);
        let signing_hash: [u8; 32] = Keccak256::digest(&msg).into();

        // 8. secp256k1 sign
        let key_bytes = hex::decode(private_key.trim_start_matches("0x"))
            .map_err(|e| format!("HL: bad private key: {}", e))?;
        let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into())
            .map_err(|e| format!("HL: invalid key: {}", e))?;
        let (sig, recid): (k256::ecdsa::Signature, k256::ecdsa::RecoveryId) = signing_key
            .sign_prehash_recoverable(&signing_hash)
            .map_err(|e| format!("HL: sign error: {}", e))?;

        let sig_r = sig.r().to_bytes();
        let sig_s = sig.s().to_bytes();
        let r_hex = format!("0x{}", hex::encode(&sig_r[..]));
        let s_hex = format!("0x{}", hex::encode(&sig_s[..]));
        let v = recid.to_byte() as u64 + 27;

        // 9. POST to exchange
        let action_json = serde_json::to_value(&action)
            .map_err(|e| format!("HL: action json: {}", e))?;
        let body = serde_json::json!({
            "action":    action_json,
            "nonce":     nonce,
            "signature": { "r": r_hex, "s": s_hex, "v": v }
        });

        let resp = client
            .post("https://api.hyperliquid.xyz/exchange")
            .json(&body)
            .send().await.map_err(|e| format!("HL send: {}", e))?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HL order failed {}: {}", status, text));
        }
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        if parsed["status"].as_str() != Some("ok") {
            return Err(format!("HL order rejected: {}", text));
        }

        info!("   -> Hyperliquid Perp {} {} size={}", side, coin, size);
        Ok(())
    }
}
