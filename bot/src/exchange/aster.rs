use super::Exchange;
use crate::config::ExchangeEndpointConfig;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use log::{error, info};
use rust_decimal::Decimal;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;

pub struct AsterLauncher {
    rest_url: String,
    poll_interval_ms: u64,
}

impl AsterLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://api.aster.finance".into()),
            poll_interval_ms: ep.poll_interval_ms.unwrap_or(500),
        }
    }
}

#[async_trait]
impl Exchange for AsterLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();
        let rest_url = self.rest_url.clone();
        let poll_interval_ms = self.poll_interval_ms;

        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default();

            let mut symbols: Vec<String> = Vec::new();
            for attempt in 1..=5 {
                info!("Aster: fetching markets (attempt {})...", attempt);
                match client.get(format!("{}/v1/markets", rest_url)).send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            let list = json["markets"].as_array()
                                .or_else(|| json["data"].as_array())
                                .or_else(|| json.as_array());
                            if let Some(list) = list {
                                symbols = list.iter().filter_map(|m| {
                                    m["symbol"].as_str()
                                        .or_else(|| m["name"].as_str())
                                        .or_else(|| m["ticker"].as_str())
                                        .map(str::to_owned)
                                }).collect();
                            }
                        }
                    }
                    Err(e) => error!("Aster: market fetch error: {}", e),
                }
                if !symbols.is_empty() { break; }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }

            info!("Aster: {} markets discovered.", symbols.len());

            let mut error_streak: u32 = 0;
            loop {
                let mut any_success = false;
                for sym in &symbols {
                    match client.get(format!("{}/v1/orderbook?symbol={}&depth=20", rest_url, sym)).send().await {
                        Ok(resp) => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(ticker) = parse_orderbook(&json, sym) {
                                    let _ = tx_clone.send(ticker).await;
                                    any_success = true;
                                }
                            }
                        }
                        Err(e) => error!("Aster: orderbook error for {}: {}", sym, e),
                    }
                }

                if any_success { error_streak = 0; } else { error_streak += 1; }

                let sleep_ms = if error_streak > 0 {
                    (poll_interval_ms * 2u64.pow(error_streak.min(6))).min(30_000)
                } else {
                    poll_interval_ms
                };
                tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;
            }
        });

        Ok(())
    }

    async fn subscribe(
        &mut self,
        _symbols: &[String],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

fn parse_orderbook(json: &serde_json::Value, sym: &str) -> Option<UnifiedTicker> {
    let parse_side = |key: &str| -> Vec<(Decimal, Decimal)> {
        json.get(key).and_then(|v| v.as_array())
            .map(|entries| {
                entries.iter().take(5).filter_map(|e| {
                    if let Some(arr) = e.as_array() {
                        if arr.len() >= 2 {
                            let p = arr[0].as_str().and_then(|s| Decimal::from_str(s).ok())
                                .or_else(|| arr[0].as_f64().and_then(|f| Decimal::from_str(&f.to_string()).ok()))?;
                            let q = arr[1].as_str().and_then(|s| Decimal::from_str(s).ok())
                                .or_else(|| arr[1].as_f64().and_then(|f| Decimal::from_str(&f.to_string()).ok()))?;
                            return if p > Decimal::ZERO && q > Decimal::ZERO { Some((p, q)) } else { None };
                        }
                    }
                    let price_str = e["price"].as_str().or_else(|| e["p"].as_str())?;
                    let size_str = e["size"].as_str()
                        .or_else(|| e["amount"].as_str())
                        .or_else(|| e["quantity"].as_str())
                        .or_else(|| e["q"].as_str())?;
                    let price = Decimal::from_str(price_str).ok()?;
                    let size = Decimal::from_str(size_str).ok()?;
                    if price > Decimal::ZERO && size > Decimal::ZERO { Some((price, size)) } else { None }
                }).collect()
            })
            .unwrap_or_default()
    };

    let bids = parse_side("bids");
    let asks = parse_side("asks");
    if bids.is_empty() || asks.is_empty() { return None; }

    Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(sym),
        exchange: ExchangeId::Aster,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids,
        asks,
    })
}
