use crate::model::{ExchangeId, FundingInfo};
use crate::rate_limiter::RateLimiter;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use std::str::FromStr;
use log::info;
use std::env;

pub struct DataPoller {
    pub funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
    pub market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
    pub account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    pub config: Arc<Mutex<crate::config::AppConfig>>,
    pub secrets: Arc<Mutex<crate::config::SecretsConfig>>,
    rate_limiter: Arc<RateLimiter>,
    client: reqwest::Client,
}

impl DataPoller {
    pub fn new(
        rate_limiter: Arc<RateLimiter>,
        config: Arc<Mutex<crate::config::AppConfig>>,
        secrets: Arc<Mutex<crate::config::SecretsConfig>>,
        client: reqwest::Client,
    ) -> Self {
        let initial_config = config.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let initial_secrets = secrets.lock().unwrap_or_else(|e| e.into_inner()).clone();
        
        Self {
            funding_rates: Arc::new(Mutex::new(HashMap::new())),
            market_filters: Arc::new(Mutex::new(HashMap::new())),
            account_state: Arc::new(Mutex::new(crate::model::GlobalAccountState {
                total_equity_usdt: Decimal::ZERO,
                total_unrealized_pnl: Decimal::ZERO,
                exchange_states: HashMap::new(),
                asset_statuses: HashMap::new(),
                config: initial_config,
                secrets: initial_secrets,
            })),
            config,
            secrets,
            rate_limiter,
            client,
        }
    }

    pub async fn run(&self) {
        loop {
            let mut new_rates = HashMap::new();
            let mut new_filters = HashMap::new();

            // 1. Fetch Binance Info & Funding & Volumes
            if self.rate_limiter.check_limit(ExchangeId::Binance, false, 2.0).await {
                let mut binance_filters = self.fetch_binance_filters().await.ok();
                let binance_vols = self.fetch_binance_volumes().await.ok();
                let binance_funding = self.fetch_binance_funding().await.ok();
                if let Some(ref mut filters) = binance_filters {
                    if let Some(vols) = binance_vols {
                        for (sym, f) in filters.iter_mut() {
                            if let Some(&vol) = vols.get(sym) {
                                f.volume_24h_usdt = vol;
                            }
                        }
                    }
                    new_filters.insert(ExchangeId::Binance, binance_filters.unwrap());
                }
                if let Some(rates) = binance_funding {
                    new_rates.insert(ExchangeId::Binance, rates);
                }
            }

            // 2. Fetch Bybit Info & Funding & Volumes
            if self.rate_limiter.check_limit(ExchangeId::Bybit, false, 2.0).await {
                let mut bybit_filters = self.fetch_bybit_filters().await.ok();
                let bybit_vols = self.fetch_bybit_volumes().await.ok();
                let bybit_funding = self.fetch_bybit_funding().await.ok();
                if let Some(ref mut filters) = bybit_filters {
                    if let Some(vols) = bybit_vols {
                        for (sym, f) in filters.iter_mut() {
                            if let Some(&vol) = vols.get(sym) {
                                f.volume_24h_usdt = vol;
                            }
                        }
                    }
                    new_filters.insert(ExchangeId::Bybit, bybit_filters.unwrap());
                }
                if let Some(rates) = bybit_funding {
                    new_rates.insert(ExchangeId::Bybit, rates);
                }
            }

            // 3. Extra Exchanges (Simplified Filters)
            for eid in &[ExchangeId::Gate] {
                if self.rate_limiter.check_limit(*eid, false, 2.0).await {
                    if let Ok(filters) = self.fetch_generic_filters(*eid).await {
                        new_filters.insert(*eid, filters);
                    }
                }
            }

            {
                let mut current_rates = self.funding_rates.lock().unwrap();
                *current_rates = new_rates;
                let mut current_filters = self.market_filters.lock().unwrap();
                *current_filters = new_filters;
            }

            info!("Funding rates and Market filters updated.");
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await; 
        }
    }

    pub async fn run_private(&self) {
        loop {
            self.fetch_private_data().await;
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    async fn fetch_binance_filters(&self) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://fapi.binance.com/fapi/v1/exchangeInfo").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(symbols) = json["symbols"].as_array() {
            for s in symbols {
                let symbol_name = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                let is_trading = s["status"].as_str() == Some("TRADING");
                
                if let Some(filters) = s["filters"].as_array() {
                    for f in filters {
                        if f["filterType"] == "NOTIONAL" || f["filterType"] == "MIN_NOTIONAL" {
                            let min_notional_str = f["minNotional"].as_str().or(f["notional"].as_str()).unwrap_or("0");
                            if let Ok(val) = Decimal::from_str(min_notional_str) {
                                map.insert(symbol_name.clone(), crate::model::SymbolMarketFilters {
                                    min_notional: val,
                                    is_trading,
                                    volume_24h_usdt: Decimal::ZERO,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(map)
    }

    async fn fetch_bybit_filters(&self) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://api.bybit.com/v5/market/instruments-info?category=linear").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(list) = json["result"]["list"].as_array() {
            for item in list {
                if let Some(s) = item["symbol"].as_str() {
                    let symbol_name = crate::model::normalize_symbol(s);
                    let is_trading = item["status"].as_str() == Some("Trading");
                    let min_notional = item["minNotionalValue"].as_str().unwrap_or("0");
                    if let Ok(val) = Decimal::from_str(min_notional) {
                        map.insert(symbol_name, crate::model::SymbolMarketFilters {
                            min_notional: val,
                            is_trading,
                            volume_24h_usdt: Decimal::ZERO,
                        });
                    }
                }
            }
        }
        Ok(map)
    }


    async fn fetch_binance_funding(&self) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://fapi.binance.com/fapi/v1/premiumIndex").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(arr) = json.as_array() {
            for item in arr {
                if let (Some(s), Some(r)) = (item["symbol"].as_str(), item["lastFundingRate"].as_str()) {
                    let symbol_name = crate::model::normalize_symbol(s);
                    if let Ok(rate) = Decimal::from_str(r) {
                        map.insert(symbol_name, FundingInfo {
                            rate_pct: rate * Decimal::from(100),
                            next_funding_time: item["nextFundingTime"].as_i64().unwrap_or(0),
                        });
                    }
                }
            }
        }
        Ok(map)
    }

    async fn fetch_bybit_funding(&self) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://api.bybit.com/v5/market/tickers?category=linear").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(list) = json["result"]["list"].as_array() {
            for item in list {
                if let (Some(s), Some(r)) = (item["symbol"].as_str(), item["fundingRate"].as_str()) {
                    let symbol_name = crate::model::normalize_symbol(s);
                    if let Ok(rate) = Decimal::from_str(r) {
                        map.insert(symbol_name, FundingInfo {
                            rate_pct: rate * Decimal::from(100),
                            next_funding_time: item["nextFundingTime"].as_str().and_then(|t| t.parse().ok()).unwrap_or(0),
                        });
                    }
                }
            }
        }
        Ok(map)
    }

    async fn fetch_private_data(&self) {
        let mut exchange_states = HashMap::new();
        let mut total_equity = Decimal::ZERO;
        let mut total_pnl = Decimal::ZERO;
        let mut asset_statuses = HashMap::new();

        // Check asset status (USDT primarily)
        if let Ok(st) = self.fetch_binance_asset_status().await { asset_statuses.insert(ExchangeId::Binance, st); }
        if let Ok(st) = self.fetch_bybit_asset_status().await { asset_statuses.insert(ExchangeId::Bybit, st); }

        // 1. Binance
        if let (Ok(key), Ok(secret)) = (env::var("BINANCE_API_KEY"), env::var("BINANCE_API_SECRET")) {
            if let Ok(state) = self.fetch_binance_account(&key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Binance, state);
            }
        }

        // 2. Bybit
        if let (Ok(key), Ok(secret)) = (env::var("BYBIT_API_KEY"), env::var("BYBIT_API_SECRET")) {
            if let Ok(state) = self.fetch_bybit_account(&key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Bybit, state);
            }
        }

        let mut current_state = self.account_state.lock().unwrap();
        current_state.total_equity_usdt = total_equity;
        current_state.total_unrealized_pnl = total_pnl;
        current_state.exchange_states = exchange_states;
        current_state.asset_statuses = asset_statuses;
        current_state.config = self.config.lock().unwrap().clone();
    }

    async fn fetch_binance_account(&self, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let query = format!("timestamp={}", timestamp);
        let signature = self._hmac_signature(secret, &query);
        let url = format!("https://fapi.binance.com/fapi/v2/account?{}&signature={}", query, signature);

        let resp = self.client.get(&url)
            .header("X-MBX-APIKEY", key)
            .send().await?;
        
        let json: serde_json::Value = resp.json().await?;
        
        let total_equity = Decimal::from_str(json["totalMarginBalance"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(json["availableBalance"].as_str().unwrap_or("0"))?;
        
        let mut positions = Vec::new();
        if let Some(pos_list) = json["positions"].as_array() {
            for p in pos_list {
                let amt = Decimal::from_str(p["positionAmt"].as_str().unwrap_or("0"))?;
                if !amt.is_zero() {
                    positions.push(crate::model::PositionInfo {
                        symbol: p["symbol"].as_str().unwrap_or("").to_string(),
                        side: if amt.is_sign_positive() { "LONG".to_string() } else { "SHORT".to_string() },
                        size: amt.abs(),
                        entry_price: Decimal::from_str(p["entryPrice"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["unrealizedProfit"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: Decimal::ZERO, // Binance uses different logic for ratio
            positions,
        })
    }

    async fn fetch_bybit_account(&self, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let query_params = "accountType=UNIFIED";
        let recv_window = "5000";
        let payload = format!("{}{}{}{}", timestamp, key, recv_window, query_params);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bybit.com/v5/account/wallet-balance?{}", query_params);
        let resp = self.client.get(&url)
            .header("X-BAPI-API-KEY", key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let res = &json["result"]["list"][0];
        
        let total_equity = Decimal::from_str(res["totalEquity"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(res["availableBalance"].as_str().unwrap_or("0"))?;

        if let Ok(pos) = self.fetch_bybit_positions(key, secret).await {
            Ok(crate::model::ExchangeAccountState {
                total_equity,
                available_balance: available,
                margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity }, 
                positions: pos,
            })
        } else {
            Ok(crate::model::ExchangeAccountState {
                total_equity,
                available_balance: available,
                margin_ratio: Decimal::ZERO,
                positions: Vec::new(),
            })
        }
    }

    async fn fetch_bybit_positions(&self, key: &str, secret: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        let query_params = "category=linear&settleCoin=USDT";
        let payload = format!("{}{}{}{}", timestamp, key, recv_window, query_params);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bybit.com/v5/position/list?{}", query_params);
        let resp = self.client.get(&url)
            .header("X-BAPI-API-KEY", key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut positions = Vec::new();

        if let Some(list) = json["result"]["list"].as_array() {
            for p in list {
                let size = Decimal::from_str(p["size"].as_str().unwrap_or("0"))?;
                if !size.is_zero() {
                    positions.push(crate::model::PositionInfo {
                        symbol: p["symbol"].as_str().unwrap_or("").to_string(),
                        side: p["side"].as_str().unwrap_or("").to_uppercase(),
                        size: size.abs(),
                        entry_price: Decimal::from_str(p["avgPrice"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["unrealisedPnl"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }
        Ok(positions)
    }

    fn _hmac_signature(&self, secret: &str, payload: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    async fn fetch_generic_filters(&self, eid: ExchangeId) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let url = match eid {
            ExchangeId::Gate => "https://api.gateio.ws/api/v4/spot/currency_pairs",
            _ => return Ok(HashMap::new()),
        };

        let resp = self.client.get(url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let ExchangeId::Gate = eid {
            if let Some(arr) = json.as_array() {
                for item in arr {
                    let symbol = crate::model::normalize_symbol(item["name"].as_str().unwrap_or(item["id"].as_str().unwrap_or("")));
                    map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(1), is_trading: true, volume_24h_usdt: Decimal::ZERO });
                }
            }
        }

        Ok(map)
    }

    /// Returns symbol -> 24h quote volume (USDT) for Binance futures.
    async fn fetch_binance_volumes(&self) -> Result<HashMap<String, Decimal>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://fapi.binance.com/fapi/v1/ticker/24hr").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();
        if let Some(arr) = json.as_array() {
            for item in arr {
                if let (Some(sym), Some(vol_str)) = (item["symbol"].as_str(), item["quoteVolume"].as_str()) {
                    if let Ok(vol) = Decimal::from_str(vol_str) {
                        map.insert(crate::model::normalize_symbol(sym), vol);
                    }
                }
            }
        }
        Ok(map)
    }

    /// Returns symbol -> 24h turnover (USDT) for Bybit linear futures.
    async fn fetch_bybit_volumes(&self) -> Result<HashMap<String, Decimal>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://api.bybit.com/v5/market/tickers?category=linear").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();
        if let Some(list) = json["result"]["list"].as_array() {
            for item in list {
                if let Some(sym) = item["symbol"].as_str() {
                    let vol_str = item["turnover24h"].as_str().unwrap_or("0");
                    if let Ok(vol) = Decimal::from_str(vol_str) {
                        map.insert(crate::model::normalize_symbol(sym), vol);
                    }
                }
            }
        }
        Ok(map)
    }

    async fn fetch_binance_asset_status(&self) -> Result<HashMap<String, crate::model::AssetStatus>, Box<dyn std::error::Error>> {
        let url = "https://fapi.binance.com/fapi/v1/exchangeInfo";
        let resp = self.client.get(url).send().await?;
        let _json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();
        map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
        Ok(map)
    }

    async fn fetch_bybit_asset_status(&self) -> Result<HashMap<String, crate::model::AssetStatus>, Box<dyn std::error::Error>> {
        let url = "https://api.bybit.com/v5/asset/coin/query-info?coin=USDT";
        let mut map = HashMap::new();
        let secrets = self.secrets.lock().unwrap().clone();
        if let (Some(key), Some(secret)) = (&secrets.bybit_key, &secrets.bybit_secret) {
            let timestamp = chrono::Utc::now().timestamp_millis().to_string();
            let query = "coin=USDT";
            let payload = format!("{}{}{}5000{}", timestamp, key, "5000", query);
            let sig = self._hmac_signature(secret, &payload);
            let resp = self.client.get(url)
                .header("X-BAPI-API-KEY", key)
                .header("X-BAPI-TIMESTAMP", timestamp)
                .header("X-BAPI-SIGN", sig)
                .header("X-BAPI-RECV-WINDOW", "5000")
                .send().await?;
            let json: serde_json::Value = resp.json().await?;
            if let Some(rows) = json["result"]["rows"].as_array() {
                for r in rows {
                    if r["coin"] == "USDT" {
                        let can_dep = r["canDeposit"].as_str() == Some("1");
                        let can_with = r["canWithdraw"].as_str() == Some("1");
                        map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: can_dep, can_withdraw: can_with, is_active: true });
                    }
                }
            }
        }
        if map.is_empty() {
             map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
        }
        Ok(map)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_poller_initialization() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let secrets = Arc::new(Mutex::new(crate::config::SecretsConfig::default()));
        let rate_limiter = Arc::new(RateLimiter::new());
        let client = reqwest::Client::new();
        let poller = DataPoller::new(rate_limiter, config, secrets, client);
        
        let state = poller.account_state.lock().unwrap();
        assert_eq!(state.total_equity_usdt, Decimal::ZERO);
        assert!(state.exchange_states.is_empty());
    }
}
