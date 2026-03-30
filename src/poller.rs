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

            // 1. Fetch Binance Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Binance, false, 2.0).await {
                if let Ok(filters) = self.fetch_binance_filters().await {
                    new_filters.insert(ExchangeId::Binance, filters);
                }
                if let Ok(rates) = self.fetch_binance_funding().await {
                    new_rates.insert(ExchangeId::Binance, rates);
                }
            }

            // 2. Fetch Bybit Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Bybit, false, 2.0).await {
                if let Ok(filters) = self.fetch_bybit_filters().await {
                    new_filters.insert(ExchangeId::Bybit, filters);
                }
                if let Ok(rates) = self.fetch_bybit_funding().await {
                    new_rates.insert(ExchangeId::Bybit, rates);
                }
            }

            // 3. Fetch Bitget Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Bitget, false, 2.0).await {
                if let Ok(filters) = self.fetch_bitget_filters().await {
                    new_filters.insert(ExchangeId::Bitget, filters);
                }
                if let Ok(rates) = self.fetch_bitget_funding().await {
                    new_rates.insert(ExchangeId::Bitget, rates);
                }
            }

            // 4. Extra Exchanges (Simplified Filters)
            for eid in &[ExchangeId::MEXC, ExchangeId::Bitmart, ExchangeId::Gate, ExchangeId::Kraken] {
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
                        });
                    }
                }
            }
        }
        Ok(map)
    }

    async fn fetch_bitget_filters(&self) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(data) = json["data"].as_array() {
            for item in data {
                if let Some(s) = item["symbol"].as_str() {
                    let symbol_name = crate::model::normalize_symbol(s);
                    let is_trading = item["symbolStatus"].as_str() == Some("normal");
                    let min_notional = item["minNotionalUsdt"].as_str().unwrap_or("0");
                    if let Ok(val) = Decimal::from_str(min_notional) {
                        map.insert(symbol_name, crate::model::SymbolMarketFilters { 
                            min_notional: val,
                            is_trading,
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

    async fn fetch_bitget_funding(&self) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = self.client.get("https://api.bitget.com/api/v2/mix/market/tickers?productType=USDT-FUTURES").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(data) = json["data"].as_array() {
            for item in data {
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
        if let Ok(st) = self.fetch_bitget_asset_status().await { asset_statuses.insert(ExchangeId::Bitget, st); }
        if let Ok(st) = self.fetch_mexc_asset_status().await { asset_statuses.insert(ExchangeId::MEXC, st); }
        // if let Ok(st) = self.fetch_okx_asset_status().await { asset_statuses.insert(ExchangeId::Okx, st); }

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

        // 3. Bitget
        if let (Ok(key), Ok(secret)) = (env::var("BITGET_API_KEY"), env::var("BITGET_API_SECRET")) {
            if let Ok(state) = self.fetch_bitget_account(&key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Bitget, state);
            }
        }

        // 4. MEXC
        if let (Ok(key), Ok(secret)) = (env::var("MEXC_API_KEY"), env::var("MEXC_API_SECRET")) {
            if let Ok(state) = self.fetch_mexc_account(&key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::MEXC, state);
            }
        }

        // 5. OKX
        if let (Ok(key), Ok(secret), Ok(passphrase)) = (env::var("OKX_API_KEY"), env::var("OKX_API_SECRET"), env::var("OKX_API_PASSPHRASE")) {
            if let Ok(state) = self.fetch_okx_account(&key, &secret, &passphrase).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Okx, state);
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

    async fn fetch_bitget_account_v2(&self, client: &reqwest::Client, key: &str, secret: &str, passphrase: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "GET";
        let request_path = "/api/v2/mix/account/accounts?productType=USDT-FUTURES";
        let payload = format!("{}{}{}", timestamp, method, request_path);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bitget.com{}", request_path);
        let resp = client.get(&url)
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase) 
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let data = &json["data"][0];

        let total_equity = Decimal::from_str(data["marginBalance"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(data["available"].as_str().unwrap_or("0"))?;

        let mut positions = Vec::new();
        if let Ok(pos) = self.fetch_bitget_positions_v2(client, key, secret, passphrase).await {
            positions = pos;
        }

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity },
            positions,
        })
    }

    async fn fetch_bitget_positions_v2(&self, client: &reqwest::Client, key: &str, secret: &str, passphrase: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let method = "GET";
        let request_path = "/api/v2/mix/position/all-position?productType=USDT-FUTURES";
        let payload = format!("{}{}{}", timestamp, method, request_path);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bitget.com{}", request_path);
        let resp = client.get(&url)
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", passphrase) 
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut positions = Vec::new();

        if let Some(list) = json["data"].as_array() {
            for p in list {
                let hold_side = p["holdSide"].as_str().unwrap_or("");
                let size = Decimal::from_str(p["total"].as_str().unwrap_or("0"))?;
                if !size.is_zero() {
                    positions.push(crate::model::PositionInfo {
                        symbol: p["symbol"].as_str().unwrap_or("").to_string(),
                        side: if hold_side == "long" { "LONG".to_string() } else { "SHORT".to_string() },
                        size: size.abs(),
                        entry_price: Decimal::from_str(p["averageOpenPrice"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["unrealizedPL"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }
        Ok(positions)
    }

    async fn fetch_bitget_account(&self, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let url = "https://api.bitget.com/api/v2/mix/account/accounts?productType=USDT-FUTURES";
        let signature = self._hmac_signature(secret, &format!("{}GET/api/v2/mix/account/accounts?productType=USDT-FUTURES", timestamp));

        let resp = self.client.get(url)
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", "YOUR_PASSPHRASE") // Bitget usually needs passphrase
            .send().await?;
        
        let json: serde_json::Value = resp.json().await?;
        let entry = &json["data"][0];
        let total_equity = Decimal::from_str(entry["marginBalance"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(entry["available"].as_str().unwrap_or("0"))?;

        let positions = self.fetch_bitget_positions(key, secret).await?;

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity },
            positions,
        })
    }

    async fn fetch_bitget_positions(&self, key: &str, secret: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let url = "https://api.bitget.com/api/v2/mix/position/all-position?productType=USDT-FUTURES";
        let signature = self._hmac_signature(secret, &format!("{}GET/api/v2/mix/position/all-position?productType=USDT-FUTURES", timestamp));

        let resp = self.client.get(url)
            .header("ACCESS-KEY", key)
            .header("ACCESS-SIGN", signature)
            .header("ACCESS-TIMESTAMP", &timestamp)
            .header("ACCESS-PASSPHRASE", "YOUR_PASSPHRASE") 
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut positions = Vec::new();

        if let Some(list) = json["data"].as_array() {
            for p in list {
                let hold_side = p["holdSide"].as_str().unwrap_or("");
                let size = Decimal::from_str(p["total"].as_str().unwrap_or("0"))?;
                if !size.is_zero() {
                    positions.push(crate::model::PositionInfo {
                        symbol: p["symbol"].as_str().unwrap_or("").to_string(),
                        side: if hold_side == "long" { "LONG".to_string() } else { "SHORT".to_string() },
                        size: size.abs(),
                        entry_price: Decimal::from_str(p["averageOpenPrice"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["unrealizedPL"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }
        Ok(positions)
    }

    async fn fetch_mexc_account(&self, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let signature = self._mexc_signature(secret, key, &timestamp, "");
        let url = format!("https://fapi.mexc.com/api/v1/private/account/assets?timestamp={}&signature={}", timestamp, signature);

        let resp = self.client.get(&url)
            .header("ApiKey", key)
            .send().await?;
        
        let json: serde_json::Value = resp.json().await?;
        let entry = &json["data"][0];
        let total_equity = Decimal::from_str(entry["equity"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(entry["available"].as_str().unwrap_or("0"))?;

        let positions = self.fetch_mexc_positions(key, secret).await?;

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity },
            positions,
        })
    }

    async fn fetch_mexc_positions(&self, key: &str, secret: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let signature = self._mexc_signature(secret, key, &timestamp, "");
        let url = format!("https://fapi.mexc.com/api/v1/private/position/open_details?timestamp={}&signature={}", timestamp, signature);

        let resp = self.client.get(&url)
            .header("ApiKey", key)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut positions = Vec::new();

        if let Some(list) = json["data"].as_array() {
            for p in list {
                let size = Decimal::from_str(p["holdVol"].as_str().unwrap_or("0"))?;
                if !size.is_zero() {
                    let side = if p["positionType"].as_i64() == Some(1) { "LONG" } else { "SHORT" };
                    positions.push(crate::model::PositionInfo {
                        symbol: p["symbol"].as_str().unwrap_or("").to_string(),
                        side: side.to_string(),
                        size: size.abs(),
                        entry_price: Decimal::from_str(p["avgEntryPrice"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["unrealisedPnl"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }
        Ok(positions)
    }

    fn _mexc_signature(&self, secret: &str, key: &str, timestamp: &str, payload: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let sign_str = format!("{}{}{}", key, timestamp, payload);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(sign_str.as_bytes());
        hex::encode(mac.finalize().into_bytes())
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
            ExchangeId::MEXC => "https://api.mexc.com/api/v3/exchangeInfo",
            ExchangeId::Bitmart => "https://api-cloud.bitmart.com/spot/v1/symbols",
            ExchangeId::Gate => "https://api.gateio.ws/api/v4/spot/currency_pairs",
            ExchangeId::Kraken => "https://api.kraken.com/0/public/AssetPairs",
            _ => return Ok(HashMap::new()),
        };

        let resp = self.client.get(url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        match eid {
            ExchangeId::MEXC => {
                if let Some(data) = json.get("data") {
                    if let Some(arr) = data.as_array() {
                        for s in arr {
                            let symbol = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                            let state = s["state"].as_i64().unwrap_or(0);
                            map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: state == 0 });
                        }
                    } else if let Some(obj) = data.as_object() {
                        if let Some(sym) = obj.get("symbol").and_then(|s| s.as_str()) {
                            let state = obj.get("state").and_then(|v| v.as_i64()).unwrap_or(0);
                            map.insert(crate::model::normalize_symbol(sym), crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: state == 0 });
                        }
                    }
                }
            }
            ExchangeId::Bitmart => {
                if json["code"].as_i64() == Some(1000) {
                    let data = json.get("data");
                    if let Some(symbols) = data.and_then(|d| d.get("symbols")).and_then(|s| s.as_array()) {
                        for s in symbols {
                            let symbol = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                            let status = s["status"].as_str().unwrap_or("");
                            map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: status == "Trading" });
                        }
                    } else if let Some(one) = data.and_then(|d| d.as_object()) {
                        if let Some(sym) = one.get("symbol").and_then(|s| s.as_str()) {
                            let status = one.get("status").and_then(|s| s.as_str()).unwrap_or("");
                            map.insert(crate::model::normalize_symbol(sym), crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: status == "Trading" });
                        }
                    }
                }
            }
            ExchangeId::Gate => {
                if let Some(arr) = json.as_array() {
                    for item in arr {
                        let symbol = crate::model::normalize_symbol(item["name"].as_str().unwrap_or(item["id"].as_str().unwrap_or("")));
                        map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(1), is_trading: true });
                    }
                }
            }
            _ => {}
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

    async fn fetch_bitget_asset_status(&self) -> Result<HashMap<String, crate::model::AssetStatus>, Box<dyn std::error::Error>> {
        let url = "https://api.bitget.com/api/spot/v1/public/currencies";
        let resp = self.client.get(url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();
        if let Some(data) = json["data"].as_array() {
            for c in data {
                if c["coinName"] == "USDT" {
                    let can_dep = c["canDeposit"].as_str() == Some("1");
                    let can_with = c["canWithdraw"].as_str() == Some("1");
                    map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: can_dep, can_withdraw: can_with, is_active: true });
                }
            }
        }
        Ok(map)
    }

    async fn fetch_mexc_asset_status(&self) -> Result<HashMap<String, crate::model::AssetStatus>, Box<dyn std::error::Error>> {
        let url = "https://api.mexc.com/api/v3/capital/config/getall";
        let mut map = HashMap::new();
        let secrets = self.secrets.lock().unwrap().clone();
        if let (Some(key), Some(secret)) = (&secrets.mexc_key, &secrets.mexc_secret) {
             let timestamp = chrono::Utc::now().timestamp_millis().to_string();
             let query = format!("timestamp={}", timestamp);
             let sig = self._hmac_signature(secret, &query);
             let resp = self.client.get(&format!("{}?{}&signature={}", url, query, sig))
                .header("X-MEXC-APIKEY", key)
                .send().await?;
             let json: serde_json::Value = resp.json().await?;
             if let Some(list) = json.as_array() {
                 for c in list {
                     if c["coin"] == "USDT" {
                         let can_dep = c["depositWebStatus"].as_bool().unwrap_or(true);
                         let can_with = c["withdrawWebStatus"].as_bool().unwrap_or(true);
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

    async fn fetch_okx_account(&self, key: &str, secret: &str, passphrase: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let path = "/api/v5/account/balance?ccy=USDT";
        let signature = self._okx_signature(secret, &timestamp, "GET", path, "");

        let url = format!("https://www.okx.com{}", path);
        let resp = self.client.get(&url)
            .header("OK-ACCESS-KEY", key)
            .header("OK-ACCESS-SIGN", signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut total_equity = Decimal::ZERO;
        let mut available = Decimal::ZERO;

        if let Some(data) = json["data"].as_array().and_then(|a| a.first()) {
            total_equity = Decimal::from_str(data["totalEq"].as_str().unwrap_or("0"))?;
            if let Some(details) = data["details"].as_array() {
                for d in details {
                    if d["ccy"] == "USDT" {
                        available = Decimal::from_str(d["availBal"].as_str().unwrap_or("0"))?;
                        break;
                    }
                }
            }
        }

        let mut positions = Vec::new();
        if let Ok(pos) = self.fetch_okx_positions(key, secret, passphrase).await {
            positions = pos;
        }

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity },
            positions,
        })
    }

    async fn fetch_okx_positions(&self, key: &str, secret: &str, passphrase: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let path = "/api/v5/account/positions?instType=SWAP"; // Perpetual swaps
        let signature = self._okx_signature(secret, &timestamp, "GET", path, "");

        let url = format!("https://www.okx.com{}", path);
        let resp = self.client.get(&url)
            .header("OK-ACCESS-KEY", key)
            .header("OK-ACCESS-SIGN", signature)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let mut positions = Vec::new();

        if let Some(data) = json["data"].as_array() {
            for p in data {
                let size = Decimal::from_str(p["pos"].as_str().unwrap_or("0"))?;
                if !size.is_zero() {
                    positions.push(crate::model::PositionInfo {
                        symbol: p["instId"].as_str().unwrap_or("").to_string(),
                        side: p["posSide"].as_str().unwrap_or("").to_uppercase(),
                        size: size.abs(),
                        entry_price: Decimal::from_str(p["avgPx"].as_str().unwrap_or("0"))?,
                        unrealized_pnl: Decimal::from_str(p["upl"].as_str().unwrap_or("0"))?,
                    });
                }
            }
        }
        Ok(positions)
    }

    async fn fetch_okx_asset_status(&self) -> Result<HashMap<String, crate::model::AssetStatus>, Box<dyn std::error::Error>> {
        // OKX public currency info
        let url = "https://www.okx.com/api/v5/public/currencies";
        let resp = self.client.get(url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();
        if let Some(list) = json["data"].as_array() {
            for c in list {
                if c["ccy"] == "USDT" {
                    let can_dep = c["canDep"].as_bool().unwrap_or(true);
                    let can_with = c["canWd"].as_bool().unwrap_or(true);
                    map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: can_dep, can_withdraw: can_with, is_active: true });
                }
            }
        }
        if map.is_empty() {
             map.insert("USDT".to_string(), crate::model::AssetStatus { can_deposit: true, can_withdraw: true, is_active: true });
        }
        Ok(map)
    }

    fn _okx_signature(&self, secret: &str, timestamp: &str, method: &str, path: &str, body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        use base64::{Engine as _, engine::general_purpose};
        type HmacSha256 = Hmac<Sha256>;

        let sign_str = format!("{}{}{}{}", timestamp, method, path, body);
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(sign_str.as_bytes());
        general_purpose::STANDARD.encode(mac.finalize().into_bytes())
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
