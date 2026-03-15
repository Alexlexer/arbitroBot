use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;

pub struct KrakenLauncher;

const KRAKEN_FUTURES_BASE: &str = "https://futures.kraken.com/derivatives/api/v3";
const KRAKEN_DEPTH_PAIRS_LIMIT: usize = 120;
const KRAKEN_CYCLE_SLEEP_SECS: u64 = 2;

#[async_trait]
impl Exchange for KrakenLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Kraken Futures Market Data (REST depth)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_kraken_symbols(&client).await {
                Ok(s) if !s.is_empty() => s,
                Ok(_) => {
                    error!("Kraken Futures instruments returned 0 symbols");
                    vec![]
                }
                Err(e) => {
                    error!("Kraken Futures instruments: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(KRAKEN_DEPTH_PAIRS_LIMIT).collect();
            info!("Kraken Futures: polling orderbook for {} symbols", symbols.len());
            loop {
                let mut sent = 0usize;
                for sym in &symbols {
                    match fetch_kraken_orderbook_one(&client, sym).await {
                        Ok(Some((bids, asks))) => {
                            let clean = sym.strip_prefix("pf_").unwrap_or(sym).strip_prefix("fi_").unwrap_or(sym);
                            let _ = tx_clone.send(UnifiedTicker {
                                symbol: crate::model::normalize_symbol(clean),
                                exchange: ExchangeId::Kraken,
                                timestamp: chrono::Utc::now().timestamp_millis(),
                                bids,
                                asks,
                            }).await;
                            sent += 1;
                        }
                        Ok(None) => {}
                        Err(e) => {
                            if sent == 0 && sym == symbols.first().map(String::as_str).unwrap_or("") {
                                error!("Kraken Futures orderbook: {}", e);
                            }
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                }
                if sent > 0 {
                    info!("Kraken Futures: sent orderbook for {} contracts", sent);
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(KRAKEN_CYCLE_SLEEP_SECS)).await;
            }
        });

        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

/// Kraken Futures GET /orderbook returns order books for all contracts.
/// Response: { "orderBook": { "symbol": { "bids": [[price, qty]], "asks": [[price, qty]] }, ... } }
/// or { "orderBook": [ { "symbol": "...", "bids": [], "asks": [] }, ... ] }
fn level_to_str(v: &serde_json::Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = v.as_f64() {
        return Some(n.to_string());
    }
    if let Some(n) = v.as_i64() {
        return Some(n.to_string());
    }
    None
}

fn parse_orderbook_value(book: &serde_json::Value) -> Option<(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>)> {
    let bids_arr = book.get("bids").and_then(|b| b.as_array())?;
    let asks_arr = book.get("asks").and_then(|a| a.as_array())?;
    let mut bids = Vec::with_capacity(bids_arr.len().min(5));
    for b in bids_arr.iter().take(5) {
        let arr = b.as_array()?;
        if arr.len() >= 2 {
            if let (Some(ps), Some(qs)) = (level_to_str(&arr[0]), level_to_str(&arr[1])) {
                if let (Ok(price), Ok(size)) = (Decimal::from_str(&ps), Decimal::from_str(&qs)) {
                    if price > Decimal::ZERO && size > Decimal::ZERO {
                        bids.push((price, size));
                    }
                }
            }
        }
    }
    let mut asks = Vec::with_capacity(asks_arr.len().min(5));
    for a in asks_arr.iter().take(5) {
        let arr = a.as_array()?;
        if arr.len() >= 2 {
            if let (Some(ps), Some(qs)) = (level_to_str(&arr[0]), level_to_str(&arr[1])) {
                if let (Ok(price), Ok(size)) = (Decimal::from_str(&ps), Decimal::from_str(&qs)) {
                    if price > Decimal::ZERO && size > Decimal::ZERO {
                        asks.push((price, size));
                    }
                }
            }
        }
    }
    if bids.is_empty() || asks.is_empty() {
        return None;
    }
    Some((bids, asks))
}

/// Fetch tradeable symbols from /instruments (prefer pf_* perp).
async fn fetch_kraken_symbols(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/instruments", KRAKEN_FUTURES_BASE);
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    if json.get("result").and_then(|r| r.as_str()) != Some("success") {
        return Err("Kraken instruments error".into());
    }
    let list = json.get("instruments").and_then(|i| i.as_array()).ok_or("missing instruments")?;
    let mut symbols: Vec<String> = list
        .iter()
        .filter_map(|i| {
            let sym = i.get("symbol").and_then(|s| s.as_str())?;
            let tradeable = i.get("tradeable").and_then(|t| t.as_bool()).unwrap_or(false);
            let ok = tradeable
                && (sym.starts_with("pf_") || sym.starts_with("fi_") || sym.starts_with("PI_") || sym.starts_with("FI_"));
            if ok {
                Some(sym.to_string())
            } else {
                None
            }
        })
        .collect();
    symbols.sort();
    Ok(symbols)
}

/// GET /orderbook?symbol=X returns { "result": "success", "orderBook": { "bids": [...], "asks": [...] } }.
async fn fetch_kraken_orderbook_one(client: &reqwest::Client, symbol: &str) -> Result<Option<(Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>)>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/orderbook?symbol={}", KRAKEN_FUTURES_BASE, symbol);
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    if json.get("result").and_then(|r| r.as_str()) != Some("success") {
        return Ok(None);
    }
    let order_book = match json.get("orderBook").or(json.get("orderbook")) {
        Some(b) => b,
        None => return Ok(None),
    };
    Ok(parse_orderbook_value(order_book))
}
