use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;
use serde::Deserialize;

pub struct GateLauncher;

const GATE_DEPTH_SYMBOLS_LIMIT: usize = 120;
const GATE_DEPTH_DELAY_MS: u64 = 200;
const GATE_CYCLE_SLEEP_SECS: u64 = 2;
const GATE_FUTURES_BASE: &str = "https://fx-api.gateio.ws/api/v4";

#[async_trait]
impl Exchange for GateLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Gate.io Futures Market Data (REST depth)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let symbols = match fetch_gate_futures_contracts(&client).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Gate: failed to fetch futures contracts: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(GATE_DEPTH_SYMBOLS_LIMIT).collect();
            info!("Gate: polling futures depth (limit 5) for {} contracts", symbols.len());

            loop {
                for contract in &symbols {
                    match fetch_gate_futures_depth(&client, contract).await {
                        Ok(Some(ticker)) => {
                            if tx_clone.send(ticker).await.is_err() {
                                error!("Gate: channel closed");
                                return;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => error!("Gate futures depth {}: {}", contract, e),
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(GATE_DEPTH_DELAY_MS)).await;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(GATE_CYCLE_SLEEP_SECS)).await;
            }
        });

        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

async fn fetch_gate_futures_contracts(client: &reqwest::Client) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/futures/usdt/contracts", GATE_FUTURES_BASE);
    let list: Vec<serde_json::Value> = client.get(&url).send().await?.json().await?;
    let symbols: Vec<String> = list
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(|n| n.as_str())?;
            if name.ends_with("_USDT") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();
    Ok(symbols)
}

#[derive(Deserialize)]
struct GateFuturesOrderBookLevel {
    p: String,
    s: i64,
}

#[derive(Deserialize)]
struct GateFuturesDepthResponse {
    bids: Vec<GateFuturesOrderBookLevel>,
    asks: Vec<GateFuturesOrderBookLevel>,
}

async fn fetch_gate_futures_depth(client: &reqwest::Client, contract: &str) -> Result<Option<UnifiedTicker>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/futures/usdt/order_book?contract={}&limit=5", GATE_FUTURES_BASE, contract);
    let resp = client.get(&url).send().await?;
    let data: GateFuturesDepthResponse = resp.json().await?;

    let mut bids_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.bids.len());
    for b in &data.bids {
        if let Ok(p) = Decimal::from_str(&b.p) {
            if p > Decimal::ZERO && b.s > 0 {
                bids_vec.push((p, Decimal::from(b.s)));
            }
        }
    }
    let mut asks_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(data.asks.len());
    for a in &data.asks {
        if let Ok(p) = Decimal::from_str(&a.p) {
            if p > Decimal::ZERO && a.s > 0 {
                asks_vec.push((p, Decimal::from(a.s)));
            }
        }
    }
    if bids_vec.is_empty() || asks_vec.is_empty() {
        return Ok(None);
    }
    Ok(Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(contract),
        exchange: ExchangeId::Gate,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    }))
}
