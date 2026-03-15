use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;
use serde::Deserialize;

pub struct BitmartLauncher;

const BITMART_DEPTH_SYMBOLS_LIMIT: usize = 120;
const BITMART_DEPTH_DELAY_MS: u64 = 200;
const BITMART_CYCLE_SLEEP_SECS: u64 = 2;
const BITMART_FUTURES_BASE: &str = "https://api-cloud-v2.bitmart.com/contract/public";

#[async_trait]
impl Exchange for BitmartLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Bitmart Futures Market Data (REST depth)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_bitmart_futures_symbols(&client).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Bitmart: failed to fetch futures contracts: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(BITMART_DEPTH_SYMBOLS_LIMIT).collect();
            info!("Bitmart: polling futures depth for {} contracts", symbols.len());

            loop {
                for sym in &symbols {
                    match fetch_bitmart_futures_depth(&client, sym).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("Bitmart: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("Bitmart futures depth {}: {}", sym, e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(BITMART_DEPTH_DELAY_MS)).await;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(BITMART_CYCLE_SLEEP_SECS)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn fetch_bitmart_futures_symbols(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/details", BITMART_FUTURES_BASE);
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

async fn fetch_bitmart_futures_depth(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/depth?symbol={}", BITMART_FUTURES_BASE, symbol);
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
