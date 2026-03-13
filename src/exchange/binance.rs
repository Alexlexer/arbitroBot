use super::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use futures_util::StreamExt;
use log::{error, info};
use rust_decimal::Decimal;
use serde::Deserialize;

use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct BinanceLauncher;

/// Max symbols in one combined stream URL (URL length limit ~2048).
const BINANCE_DEPTH_SYMBOLS_LIMIT: usize = 80;

#[async_trait]
impl Exchange for BinanceLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let mut reconnect_attempt: u32 = 0;

            // Fetch perpetual USDT symbols from Binance Futures
            let symbols = match fetch_binance_perpetual_symbols().await {
                Ok(s) => s,
                Err(e) => {
                    error!("Binance: failed to fetch symbols: {}", e);
                    vec!["BTCUSDT".into(), "ETHUSDT".into()]
                }
            };
            let symbols: Vec<String> = symbols
                .into_iter()
                .take(BINANCE_DEPTH_SYMBOLS_LIMIT)
                .collect();
            info!("Binance: subscribing to depth5 for {} symbols", symbols.len());

            let stream_path = symbols
                .iter()
                .map(|s| format!("{}@depth5@100ms", s.to_lowercase()))
                .collect::<Vec<_>>()
                .join("/");
            let url_str = format!(
                "wss://fstream.binance.com/stream?streams={}",
                stream_path
            );
            let url = match Url::parse(&url_str) {
                Ok(u) => u,
                Err(e) => {
                    error!("Binance: invalid depth URL: {}", e);
                    return;
                }
            };

            loop {
                info!("Connecting to Binance Futures (depth5)...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to Binance Futures depth5.");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(wrapper) =
                                    serde_json::from_str::<BinanceCombinedMessage>(&text)
                                {
                                    if let Some(ticker) = parse_depth_update(&wrapper.data) {
                                        if tx_clone.send(ticker).await.is_err() {
                                            error!("Binance: channel closed");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Binance connection error: {}", e);
                        reconnect_attempt += 1;
                    }
                }
                let delay = super::reconnect_delay_secs(reconnect_attempt);
                tokio::time::sleep(Duration::from_secs(delay)).await;
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

async fn fetch_binance_perpetual_symbols() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send()
        .await?;
    let json: serde_json::Value = resp.json().await?;
    let list = json
        .get("symbols")
        .and_then(|s| s.as_array())
        .ok_or("missing symbols")?;
    let symbols: Vec<String> = list
        .iter()
        .filter(|s| {
            s.get("contractType").and_then(|c| c.as_str()) == Some("PERPETUAL")
                && s.get("quoteAsset").and_then(|q| q.as_str()) == Some("USDT")
                && s.get("status").and_then(|st| st.as_str()) == Some("TRADING")
        })
        .filter_map(|s| s.get("symbol").and_then(|sym| sym.as_str()).map(String::from))
        .collect();
    Ok(symbols)
}

/// Combined stream wrapper: {"stream":"btcusdt@depth5@100ms","data":{...}}
#[derive(Deserialize)]
struct BinanceCombinedMessage {
    data: BinanceDepthUpdate,
}

#[derive(Deserialize)]
struct BinanceDepthUpdate {
    #[serde(rename = "e")]
    _event: String,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "b")]
    bids: Vec<[String; 2]>,
    #[serde(rename = "a")]
    asks: Vec<[String; 2]>,
}

fn parse_depth_update(data: &BinanceDepthUpdate) -> Option<UnifiedTicker> {
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
        return None;
    }
    Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(&data.symbol),
        exchange: ExchangeId::Binance,
        timestamp: chrono::Utc::now().timestamp_millis(),
        bids: bids_vec,
        asks: asks_vec,
    })
}
