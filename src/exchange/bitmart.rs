use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;
use serde::Deserialize;

pub struct BitmartLauncher;

const BITMART_DEPTH_SYMBOLS_LIMIT: usize = 60;
const BITMART_DEPTH_DELAY_MS: u64 = 200;
const BITMART_CYCLE_SLEEP_SECS: u64 = 2;

#[async_trait]
impl Exchange for BitmartLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Bitmart Market Data (REST depth)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_bitmart_symbols(&client).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Bitmart: failed to fetch symbols: {}", e);
                    vec!["BTC_USDT".into(), "ETH_USDT".into()]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(BITMART_DEPTH_SYMBOLS_LIMIT).collect();
            info!("Bitmart: polling depth (limit 5) for {} symbols", symbols.len());

            loop {
                for sym in &symbols {
                    match fetch_bitmart_depth(&client, sym).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("Bitmart: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("Bitmart depth {}: {}", sym, e),
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

async fn fetch_bitmart_symbols(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get("https://api-cloud.bitmart.com/spot/v1/ticker").send().await?;
    let json: serde_json::Value = resp.json().await?;
    let tickers = json["data"]["tickers"].as_array().ok_or("missing tickers")?;
    let symbols: Vec<String> = tickers
        .iter()
        .filter_map(|t| t["symbol"].as_str())
        .filter(|s| s.ends_with("_USDT"))
        .map(String::from)
        .collect();
    Ok(symbols)
}

#[derive(Deserialize)]
struct BitmartDepthData {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

#[derive(Deserialize)]
struct BitmartDepthResponse {
    data: BitmartDepthData,
}

async fn fetch_bitmart_depth(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://api-cloud.bitmart.com/spot/quotation/v3/books?symbol={}&limit=5", symbol);
    let resp = client.get(&url).send().await?;
    let wrapper: BitmartDepthResponse = resp.json().await?;
    let data = &wrapper.data;

    let mut bids_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.bids.len());
    for b in &data.bids {
        if let (Ok(p), Ok(q)) = (Decimal::from_str(&b[0]), Decimal::from_str(&b[1])) {
            if p > Decimal::ZERO && q > Decimal::ZERO {
                bids_vec.push((p, q));
            }
        }
    }
    let mut asks_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.asks.len());
    for a in &data.asks {
        if let (Ok(p), Ok(q)) = (Decimal::from_str(&a[0]), Decimal::from_str(&a[1])) {
            if p > Decimal::ZERO && q > Decimal::ZERO {
                asks_vec.push((p, q));
            }
        }
    }
    if bids_vec.is_empty() || asks_vec.is_empty() {
        return Ok(None);
    }
    Ok(Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(symbol),
        exchange: ExchangeId::Bitmart,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    }))
}
