use crate::model::{ExchangeId, FundingInfo};
use crate::rate_limiter::RateLimiter;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::str::FromStr;
use log::info;

pub struct DataPoller {
    pub funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
    pub market_filters: Arc<Mutex<HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>>>,
    rate_limiter: Arc<RateLimiter>,
}

impl DataPoller {
    pub fn new(rate_limiter: Arc<RateLimiter>) -> Self {
        Self {
            funding_rates: Arc::new(Mutex::new(HashMap::new())),
            market_filters: Arc::new(Mutex::new(HashMap::new())),
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

            {
                let mut current_rates = self.funding_rates.lock().unwrap();
                *current_rates = new_rates;
                let mut current_filters = self.market_filters.lock().unwrap();
                *current_filters = new_filters;
            }

            info!("Funding rates and Market filters updated.");
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await; // Cache filters/funding for 1h (funding is 8h anyway)
        }
    }

    async fn fetch_binance_filters(&self, client: &reqwest::Client) -> Result<HashMap<String, crate::model::SymbolMarketFilters>, Box<dyn std::error::Error>> {
        let resp = client.get("https://fapi.binance.com/fapi/v1/exchangeInfo").send().await?;
        let json: serde_json::Value = resp.json().await?;
        let mut map = HashMap::new();

        if let Some(symbols) = json["symbols"].as_array() {
            for s in symbols {
                let symbol_name = s["symbol"].as_str().unwrap_or("");
                let is_trading = s["status"].as_str() == Some("TRADING");
                
                if let Some(filters) = s["filters"].as_array() {
                    for f in filters {
                        if f["filterType"] == "NOTIONAL" || f["filterType"] == "MIN_NOTIONAL" {
                            let min_notional_str = f["minNotional"].as_str().or(f["notional"].as_str()).unwrap_or("0");
                            if let Ok(val) = Decimal::from_str(min_notional_str) {
                                map.insert(symbol_name.to_string(), crate::model::SymbolMarketFilters { 
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
                    let is_trading = item["status"].as_str() == Some("Trading");
                    let min_notional = item["minNotionalValue"].as_str().unwrap_or("0");
                    if let Ok(val) = Decimal::from_str(min_notional) {
                        map.insert(s.to_string(), crate::model::SymbolMarketFilters { 
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
                    let is_trading = item["symbolStatus"].as_str() == Some("normal");
                    let min_notional = item["minNotionalUsdt"].as_str().unwrap_or("0");
                    if let Ok(val) = Decimal::from_str(min_notional) {
                        map.insert(s.to_string(), crate::model::SymbolMarketFilters { 
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
                    if let Ok(rate) = Decimal::from_str(r) {
                        map.insert(s.to_string(), FundingInfo {
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
                    if let Ok(rate) = Decimal::from_str(r) {
                        map.insert(s.to_string(), FundingInfo {
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
                    if let Ok(rate) = Decimal::from_str(r) {
                        map.insert(s.to_string(), FundingInfo {
                            rate_pct: rate * Decimal::from(100),
                            next_funding_time: item["nextFundingTime"].as_str().and_then(|t| t.parse().ok()).unwrap_or(0),
                        });
                    }
                }
            }
        }
        Ok(map)
    }
}
