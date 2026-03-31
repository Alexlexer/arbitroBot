use super::Exchange;
use crate::config::ExchangeEndpointConfig;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use log::{error, info};
use rust_decimal::Decimal;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;

pub struct LighterLauncher {
    rest_url: String,
    poll_interval_ms: u64,
}

impl LighterLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://mainnet.zklighter.elliot.ai".into()),
            poll_interval_ms: ep.poll_interval_ms.unwrap_or(500),
        }
    }
}

#[async_trait]
impl Exchange for LighterLauncher {
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

            let mut markets: Vec<(u32, String)> = Vec::new();
            for attempt in 1..=5 {
                info!("Lighter: fetching markets (attempt {})...", attempt);
                match client.get(format!("{}/markets", rest_url)).send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(list) = json["markets"].as_array() {
                                markets = list.iter().filter_map(|m| {
                                    let id = m["market_id"].as_u64()? as u32;
                                    let base = m["base_ticker"].as_str()
                                        .or_else(|| m["base_asset"].as_str())
                                        .or_else(|| m["name"].as_str())?;
                                    Some((id, base.to_owned()))
                                }).collect();
                            }
                        }
                    }
                    Err(e) => error!("Lighter: market fetch error: {}", e),
                }
                if !markets.is_empty() { break; }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }

            info!("Lighter: {} markets discovered.", markets.len());

            let mut error_streak: u32 = 0;
            loop {
                let mut any_success = false;
                for (market_id, base) in &markets {
                    match client.get(format!("{}/orderbook/{}?depth=5", rest_url, market_id)).send().await {
                        Ok(resp) => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(ticker) = parse_orderbook(&json, base, *market_id) {
                                    let _ = tx_clone.send(ticker).await;
                                    any_success = true;
                                }
                            }
                        }
                        Err(e) => error!("Lighter: orderbook fetch error for market {}: {}", market_id, e),
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

fn parse_orderbook(json: &serde_json::Value, base: &str, _market_id: u32) -> Option<UnifiedTicker> {
    let parse_side = |key: &str| -> Vec<(Decimal, Decimal)> {
        json.get(key).and_then(|v| v.as_array())
            .map(|entries| {
                entries.iter().take(5).filter_map(|e| {
                    let price_str = e["price"].as_str().or_else(|| e["p"].as_str())?;
                    let size_str = e["base_amount"].as_str()
                        .or_else(|| e["size"].as_str())
                        .or_else(|| e["amount"].as_str())
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
        symbol: crate::model::normalize_symbol(base),
        exchange: ExchangeId::Lighter,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids,
        asks,
    })
}
