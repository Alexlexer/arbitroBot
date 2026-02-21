use crate::model::{ExchangeId, FundingInfo};
use crate::rate_limiter::RateLimiter;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::env;
use std::str::FromStr;
use log::info;

pub struct DataPoller {
    pub funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
    pub market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
    pub account_state: Arc<Mutex<crate::model::GlobalAccountState>>,
    rate_limiter: Arc<RateLimiter>,
}

impl DataPoller {
    pub fn new(rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            funding_rates: Arc::new(Mutex::new(HashMap::new())),
            market_filters: Arc::new(Mutex::new(HashMap::new())),
            account_state: Arc::new(Mutex::new(crate::model::GlobalAccountState {
                total_equity_usdt: Decimal::ZERO,
                total_unrealized_pnl: Decimal::ZERO,
                exchange_states: HashMap::new(),
            })),
            rate_limiter,
        }
    }

    pub async fn run(&self) {
        let client = reqwest::Client::new();
        loop {
            let mut new_rates = HashMap::new();
            let mut new_filters = HashMap::new();

            // 1. Fetch Binance Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Binance, false, 2.0).await {
                if let Ok(filters) = self.fetch_binance_filters(&client).await {
                    new_filters.insert(ExchangeId::Binance, filters);
                }
                if let Ok(rates) = self.fetch_binance_funding(&client).await {
                    new_rates.insert(ExchangeId::Binance, rates);
                }
            }

            // 2. Fetch Bybit Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Bybit, false, 2.0).await {
                if let Ok(filters) = self.fetch_bybit_filters(&client).await {
                    new_filters.insert(ExchangeId::Bybit, filters);
                }
                if let Ok(rates) = self.fetch_bybit_funding(&client).await {
                    new_rates.insert(ExchangeId::Bybit, rates);
                }
            }

            // 3. Fetch Bitget Info & Funding
            if self.rate_limiter.check_limit(ExchangeId::Bitget, false, 2.0).await {
                if let Ok(filters) = self.fetch_bitget_filters(&client).await {
                    new_filters.insert(ExchangeId::Bitget, filters);
                }
                if let Ok(rates) = self.fetch_bitget_funding(&client).await {
                    new_rates.insert(ExchangeId::Bitget, rates);
                }
            }

            // 4. Extra Exchanges (Simplified Filters)
            for eid in &[ExchangeId::MEXC, ExchangeId::Bitmart, ExchangeId::Gate, ExchangeId::Kraken, ExchangeId::Ourbit] {
                if self.rate_limiter.check_limit(*eid, false, 2.0).await {
                    if let Ok(filters) = self.fetch_generic_filters(&client, *eid).await {
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
        let client = reqwest::Client::new();
        loop {
            self.fetch_private_data(&client).await;
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
        }
    }

    async fn fetch_binance_filters(&self, client: &reqwest::Client) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = client.get("https://fapi.binance.com/fapi/v1/exchangeInfo").send().await?;
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

    async fn fetch_bybit_filters(&self, client: &reqwest::Client) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = client.get("https://api.bybit.com/v5/market/instruments-info?category=linear").send().await?;
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

    async fn fetch_bitget_filters(&self, client: &reqwest::Client) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = client.get("https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES").send().await?;
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

    async fn fetch_binance_funding(&self, client: &reqwest::Client) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = client.get("https://fapi.binance.com/fapi/v1/premiumIndex").send().await?;
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

    async fn fetch_bybit_funding(&self, client: &reqwest::Client) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = client.get("https://api.bybit.com/v5/market/tickers?category=linear").send().await?;
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

    async fn fetch_bitget_funding(&self, client: &reqwest::Client) -> Result<HashMap<String, FundingInfo>, Box<dyn std::error::Error>> {
        let resp = client.get("https://api.bitget.com/api/v2/mix/market/tickers?productType=USDT-FUTURES").send().await?;
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

    async fn fetch_private_data(&self, client: &reqwest::Client) {
        let mut exchange_states = HashMap::new();
        let mut total_equity = Decimal::ZERO;
        let mut total_pnl = Decimal::ZERO;

        // 1. Binance
        if let (Ok(key), Ok(secret)) = (env::var("BINANCE_API_KEY"), env::var("BINANCE_API_SECRET")) {
            if let Ok(state) = self.fetch_binance_account(client, &key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Binance, state);
            }
        }

        // 2. Bybit
        if let (Ok(key), Ok(secret)) = (env::var("BYBIT_API_KEY"), env::var("BYBIT_API_SECRET")) {
            if let Ok(state) = self.fetch_bybit_account(client, &key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Bybit, state);
            }
        }

        // 3. Bitget
        if let (Ok(key), Ok(secret)) = (env::var("BITGET_API_KEY"), env::var("BITGET_API_SECRET")) {
            if let Ok(state) = self.fetch_bitget_account(client, &key, &secret).await {
                total_equity += state.total_equity;
                total_pnl += state.positions.iter().map(|p| p.unrealized_pnl).sum::<Decimal>();
                exchange_states.insert(ExchangeId::Bitget, state);
            }
        }

        let mut current_state = self.account_state.lock().unwrap();
        current_state.total_equity_usdt = total_equity;
        current_state.total_unrealized_pnl = total_pnl;
        current_state.exchange_states = exchange_states;
    }

    async fn fetch_binance_account(&self, client: &reqwest::Client, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let query = format!("timestamp={}", timestamp);
        let signature = self._hmac_signature(secret, &query);
        let url = format!("https://fapi.binance.com/fapi/v2/account?{}&signature={}", query, signature);

        let resp = client.get(&url)
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

    async fn fetch_bybit_account(&self, client: &reqwest::Client, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        let query = "accountType=UNIFIED";
        let payload = format!("{}{}{}{}", timestamp, key, recv_window, query);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bybit.com/v5/account/wallet-balance?{}", query);
        let resp = client.get(&url)
            .header("X-BAPI-API-KEY", key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-RECV-WINDOW", recv_window)
            .header("X-BAPI-SIGN", signature)
            .send().await?;

        let json: serde_json::Value = resp.json().await?;
        let res = &json["result"]["list"][0];
        
        let total_equity = Decimal::from_str(res["totalEquity"].as_str().unwrap_or("0"))?;
        let available = Decimal::from_str(res["availableBalance"].as_str().unwrap_or("0"))?;

        if let Ok(pos) = self.fetch_bybit_positions(client, key, secret).await {
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

    async fn fetch_bybit_positions(&self, client: &reqwest::Client, key: &str, secret: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let recv_window = "5000";
        let query = "category=linear&settleCoin=USDT";
        let payload = format!("{}{}{}{}", timestamp, key, recv_window, query);
        let signature = self._hmac_signature(secret, &payload);

        let url = format!("https://api.bybit.com/v5/position/list?{}", query);
        let resp = client.get(&url)
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

    async fn fetch_bitget_account(&self, client: &reqwest::Client, key: &str, secret: &str) -> Result<crate::model::ExchangeAccountState, Box<dyn std::error::Error>> {
        let passphrase = env::var("BITGET_API_PASSPHRASE").unwrap_or_default();
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
        if let Ok(pos) = self.fetch_bitget_positions(client, key, secret).await {
            positions = pos;
        }

        Ok(crate::model::ExchangeAccountState {
            total_equity,
            available_balance: available,
            margin_ratio: if total_equity.is_zero() { Decimal::ZERO } else { (total_equity - available) / total_equity },
            positions,
        })
    }

    async fn fetch_bitget_positions(&self, client: &reqwest::Client, key: &str, secret: &str) -> Result<Vec<crate::model::PositionInfo>, Box<dyn std::error::Error>> {
        let passphrase = env::var("BITGET_API_PASSPHRASE").unwrap_or_default();
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

    fn _hmac_signature(&self, secret: &str, payload: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    async fn fetch_generic_filters(&self, client: &reqwest::Client, eid: ExchangeId) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let url = match eid {
            ExchangeId::MEXC => "https://api.mexc.com/api/v3/exchangeInfo",
            ExchangeId::Bitmart => "https://api-cloud.bitmart.com/spot/v1/symbols",
            ExchangeId::Gate => "https://api.gateio.ws/api/v4/spot/currency_pairs",
            ExchangeId::Kraken => "https://api.kraken.com/0/public/AssetPairs",
            ExchangeId::Ourbit => "https://api.ourbit.com/api/v1/exchangeInfo",
            _ => return Ok(HashMap::new()),
        };

        let resp = client.get(url).send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        match eid {
            ExchangeId::MEXC => {
                if let Some(symbols) = json["symbols"].as_array() {
                    for s in symbols {
                        let symbol = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                        map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: true });
                    }
                }
            }
            ExchangeId::Bitmart => {
                if let Some(symbols) = json["symbols"].as_array() {
                    for s in symbols {
                        let symbol = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                        let status = s["status"].as_str().unwrap_or("");
                        map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: status == "ENABLED" });
                    }
                }
            }
            ExchangeId::Gate => {
                if let Some(arr) = json.as_array() {
                    for item in arr {
                        let symbol = crate::model::normalize_symbol(item["id"].as_str().unwrap_or(""));
                        let status = item["trade_status"].as_str().unwrap_or("");
                        map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(1), is_trading: status == "tradable" });
                    }
                }
            }
            ExchangeId::Ourbit => {
                if let Some(symbols) = json["symbols"].as_array() {
                    for s in symbols {
                        let symbol = crate::model::normalize_symbol(s["symbol"].as_str().unwrap_or(""));
                        map.insert(symbol, crate::model::SymbolMarketFilters { min_notional: Decimal::from(5), is_trading: true });
                    }
                }
            }
            _ => {}
        }

        Ok(map)
    }
}
