use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;
use serde::Deserialize;

pub struct MexcLauncher;

/// Max symbols to poll for depth (rate limit: ~10 req/s; we use ~5 req/s with 200ms delay).
const MEXC_DEPTH_SYMBOLS_LIMIT: usize = 60;
const MEXC_DEPTH_DELAY_MS: u64 = 200;
const MEXC_CYCLE_SLEEP_SECS: u64 = 2;

#[async_trait]
impl Exchange for MexcLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to MEXC Market Data (REST depth)...");

        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_mexc_spot_symbols(&client).await {
                Ok(s) => s,
                Err(e) => {
                    error!("MEXC: failed to fetch symbols: {}", e);
                    vec!["BTCUSDT".into(), "ETHUSDT".into()]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(MEXC_DEPTH_SYMBOLS_LIMIT).collect();
            info!("MEXC: polling depth (limit {}) for {} symbols", 5, symbols.len());

            loop {
                for sym in &symbols {
                    match fetch_mexc_depth(&client, sym).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("MEXC: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("MEXC depth {}: {}", sym, e),
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

async fn fetch_mexc_spot_symbols(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get("https://api.mexc.com/api/v3/exchangeInfo").send().await?;
    let json: serde_json::Value = resp.json().await?;
    let list = json.get("symbols").and_then(|s| s.as_array()).ok_or("missing symbols")?;
    let symbols: Vec<String> = list
        .iter()
        .filter(|s| {
            s.get("status").and_then(|st| st.as_str()) == Some("ENABLED")
                && (s.get("symbol").and_then(|sym| sym.as_str()).map_or(false, |sym| sym.ends_with("USDT")))
        })
        .filter_map(|s| s.get("symbol").and_then(|sym| sym.as_str()).map(String::from))
        .collect();
    Ok(symbols)
}

#[derive(Deserialize)]
struct MexcDepthResponse {
    bids: Vec<[String; 2]>,
    asks: Vec<[String; 2]>,
}

async fn fetch_mexc_depth(client: &reqwest::Client, symbol: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://api.mexc.com/api/v3/depth?symbol={}&limit=5", symbol);
    let resp = client.get(&url).send().await?;
    let data: MexcDepthResponse = resp.json().await?;

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
        exchange: ExchangeId::MEXC,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    }))
}
