use crate::model::{ExchangeId, FundingInfo};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::str::FromStr;
use log::{error, info};

pub struct DataPoller {
    pub funding_rates: Arc<Mutex<HashMap<ExchangeId, HashMap<String, FundingInfo>>>>,
}

impl DataPoller {
    pub fn new() -> Self {
        Self {
            funding_rates: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&self) {
        let client = reqwest::Client::new();
        loop {
            let mut new_rates = HashMap::new();

            // 1. Fetch Binance Funding
            if let Ok(rates) = self.fetch_binance_funding(&client).await {
                new_rates.insert(ExchangeId::Binance, rates);
            }

            // 2. Fetch Bybit Funding
            if let Ok(rates) = self.fetch_bybit_funding(&client).await {
                new_rates.insert(ExchangeId::Bybit, rates);
            }

            // 3. Fetch Bitget Funding
            if let Ok(rates) = self.fetch_bitget_funding(&client).await {
                new_rates.insert(ExchangeId::Bitget, rates);
            }

            {
                let mut current = self.funding_rates.lock().unwrap();
                *current = new_rates;
            }

            info!("Funding rates updated for all exchanges.");
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // Update every 5m
        }
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
