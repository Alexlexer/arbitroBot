use async_trait::async_trait;
use crate::config::ExchangeEndpointConfig;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;
use serde::Deserialize;

pub struct BitmartLauncher {
    rest_url: String,
    max_symbols: usize,
    depth_delay_ms: u64,
    cycle_sleep_ms: u64,
}

impl BitmartLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://api-cloud-v2.bitmart.com/contract/public".into()),
            max_symbols: ep.max_symbols.unwrap_or(120),
            depth_delay_ms: ep.sub_batch_delay_ms.unwrap_or(200),
            cycle_sleep_ms: ep.poll_interval_ms.unwrap_or(2000),
        }
    }
}

#[async_trait]
impl Exchange for BitmartLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Bitmart Futures Market Data (REST depth)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        let rest_url = self.rest_url.clone();
        let max_symbols = self.max_symbols;
        let depth_delay_ms = self.depth_delay_ms;
        let cycle_sleep_ms = self.cycle_sleep_ms;

        tokio::spawn(async move {
            let symbols = match fetch_bitmart_futures_symbols(&client, &rest_url).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Bitmart: failed to fetch futures contracts: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(max_symbols).collect();
            info!("Bitmart: polling futures depth for {} contracts", symbols.len());

            loop {
                for sym in &symbols {
                    match fetch_bitmart_futures_depth(&client, &rest_url, sym).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("Bitmart: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("Bitmart futures depth {}: {}", sym, e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(depth_delay_ms)).await;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(cycle_sleep_ms)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn fetch_bitmart_futures_symbols(client: &reqwest::Client, rest_url: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/details", rest_url);
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    if json["code"].as_i64() != Some(1000) {
        return Err("Bitmart details non-OK".into());
    }
    let data = json.get("data").ok_or("missing data")?;
    let symbols: Vec<String> = if let Some(symbols_arr) = data.get("symbols").and_then(|s| s.as_array()) {
        symbols_arr
            .iter()
            .filter_map(|s| {
                let sym = s.get("symbol").and_then(|v| v.as_str())?;
                let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
                if sym.ends_with("USDT") && status == "Trading" {
                    Some(sym.to_string())
                } else {
                    None
                }
            })
            .collect()
    } else {
        data.get("symbol")
            .and_then(|s| s.as_str())
            .filter(|s| s.ends_with("USDT"))
            .map(|s| vec![s.to_string()])
            .unwrap_or_default()
    };
    Ok(symbols)
}

#[derive(Deserialize)]
struct BitmartFuturesDepthData {
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
    symbol: String,
}

#[derive(Deserialize)]
struct BitmartFuturesDepthResponse {
    code: i64,
    data: Option<BitmartFuturesDepthData>,
}

async fn fetch_bitmart_futures_depth(client: &reqwest::Client, rest_url: &str, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/depth?symbol={}", rest_url, symbol);
    let resp: BitmartFuturesDepthResponse = client.get(&url).send().await?.json().await?;
    if resp.code != 1000 {
        return Ok(None);
    }
    let data = match resp.data {
        Some(d) => d,
        None => return Ok(None),
    };
    let mut bids_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.bids.len().min(5));
    for b in data.bids.iter().take(5) {
        if b.len() >= 2 {
            if let (Ok(p), Ok(q)) = (Decimal::from_str(&b[0]), Decimal::from_str(&b[1])) {
                if p > Decimal::ZERO && q > Decimal::ZERO {
                    bids_vec.push((p, q));
                }
            }
        }
    }
    let mut asks_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.asks.len().min(5));
    for a in data.asks.iter().take(5) {
        if a.len() >= 2 {
            if let (Ok(p), Ok(q)) = (Decimal::from_str(&a[0]), Decimal::from_str(&a[1])) {
                if p > Decimal::ZERO && q > Decimal::ZERO {
                    asks_vec.push((p, q));
                }
            }
        }
    }
    if bids_vec.is_empty() || asks_vec.is_empty() {
        return Ok(None);
    }
    Ok(Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(&data.symbol),
        exchange: ExchangeId::Bitmart,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    }))
}
