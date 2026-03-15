use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;

pub struct MexcLauncher;

const MEXC_DEPTH_SYMBOLS_LIMIT: usize = 120;
const MEXC_DEPTH_DELAY_MS: u64 = 200;
const MEXC_CYCLE_SLEEP_SECS: u64 = 2;
const MEXC_FUTURES_BASE: &str = "https://api.mexc.com/api/v1/contract";

#[async_trait]
impl Exchange for MexcLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to MEXC Futures Market Data (REST depth)...");

        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_mexc_futures_contracts(&client).await {
                Ok(s) => s,
                Err(e) => {
                    error!("MEXC: failed to fetch futures contracts: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(MEXC_DEPTH_SYMBOLS_LIMIT).collect();
            info!("MEXC: polling futures depth (limit 5) for {} contracts", symbols.len());

            loop {
                for sym in &symbols {
                    match fetch_mexc_futures_depth(&client, sym).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("MEXC: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("MEXC futures depth {}: {}", sym, e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(MEXC_DEPTH_DELAY_MS)).await;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(MEXC_CYCLE_SLEEP_SECS)).await;
            }
        });

        Ok(())
    }

    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn fetch_mexc_futures_contracts(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/detail", MEXC_FUTURES_BASE);
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    let data = json.get("data").ok_or("missing data")?;
    let symbols: Vec<String> = if let Some(arr) = data.as_array() {
        arr.iter()
            .filter_map(|s| {
                let sym = s.get("symbol").and_then(|s| s.as_str())?;
                if sym.ends_with("_USDT") && s.get("state").and_then(|v| v.as_i64()) == Some(0) {
                    Some(sym.to_string())
                } else {
                    None
                }
            })
            .collect()
    } else if let Some(obj) = data.as_object() {
        obj.get("symbol")
            .and_then(|s| s.as_str())
            .filter(|s| s.ends_with("_USDT"))
            .map(|s| vec![s.to_string()])
            .unwrap_or_default()
    } else {
        vec![]
    };
    Ok(symbols)
}

fn parse_depth_level(arr: &serde_json::Value) -> Option<(Decimal, Decimal)> {
    let a = arr.as_array()?;
    let price = match a.get(0) {
        Some(serde_json::Value::Number(n)) => Decimal::from_str(&n.to_string()).ok()?,
        Some(serde_json::Value::String(s)) => Decimal::from_str(s).ok()?,
        _ => return None,
    };
    let size = match a.get(1) {
        Some(serde_json::Value::Number(n)) => Decimal::from_str(&n.to_string()).ok()?,
        Some(serde_json::Value::String(s)) => Decimal::from_str(s).ok()?,
        _ => return None,
    };
    if price > Decimal::ZERO && size > Decimal::ZERO {
        Some((price, size))
    } else {
        None
    }
}

async fn fetch_mexc_futures_depth(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/depth/{}?limit=5", MEXC_FUTURES_BASE, symbol);
    let resp = client.get(&url).send().await?;
    let json: serde_json::Value = resp.json().await?;
    if json.get("success").and_then(|v| v.as_bool()) == Some(false) {
        return Ok(None);
    }
    let empty: Vec<serde_json::Value> = vec![];
    let (bids_raw, asks_raw) = if let Some(data) = json.get("data").and_then(|d| d.as_object()) {
        (
            data.get("bids").and_then(|b| b.as_array()).unwrap_or(&empty),
            data.get("asks").and_then(|a| a.as_array()).unwrap_or(&empty),
        )
    } else {
        (
            json.get("bids").and_then(|b| b.as_array()).unwrap_or(&empty),
            json.get("asks").and_then(|a| a.as_array()).unwrap_or(&empty),
        )
    };

    let mut bids_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(bids_raw.len());
    for b in bids_raw {
        if let Some(pq) = parse_depth_level(b) {
            bids_vec.push(pq);
        }
    }
    let mut asks_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(asks_raw.len());
    for a in asks_raw {
        if let Some(pq) = parse_depth_level(a) {
            asks_vec.push(pq);
        }
    }
    if bids_vec.is_empty() || asks_vec.is_empty() {
        return Ok(None);
    }
    Ok(Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(symbol),
        exchange: ExchangeId::MEXC,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    }))
}
