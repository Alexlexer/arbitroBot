use super::Exchange;
use crate::config::ExchangeEndpointConfig;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use rust_decimal::Decimal;
use serde_json::json;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct MexcLauncher {
    ws_url: String,
    rest_url: String,
    max_symbols: usize,
    sub_batch_size: usize,
    ping_interval_secs: u64,
}

impl MexcLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            ws_url: ep.ws_url.clone().unwrap_or_else(|| "wss://contract.mexc.com/edge".into()),
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://contract.mexc.com".into()),
            max_symbols: ep.max_symbols.unwrap_or(120),
            sub_batch_size: ep.sub_batch_size.unwrap_or(20),
            ping_interval_secs: ep.ping_interval_secs.unwrap_or(20),
        }
    }
}

#[async_trait]
impl Exchange for MexcLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();
        let ws_url = self.ws_url.clone();
        let rest_url = self.rest_url.clone();
        let max_symbols = self.max_symbols;
        let sub_batch_size = self.sub_batch_size;
        let ping_interval_secs = self.ping_interval_secs;

        tokio::spawn(async move {
            let url = Url::parse(&ws_url).expect("Invalid MEXC WebSocket URL");
            let mut reconnect_attempt: u32 = 0;

            let client = reqwest::Client::new();
            let symbols = match fetch_mexc_futures_contracts(&client, &rest_url).await {
                Ok(s) => s,
                Err(e) => {
                    error!("MEXC: failed to fetch futures contracts: {}", e);
                    vec![]
                }
            };
            let symbols: Vec<String> = symbols.into_iter().take(max_symbols).collect();
            info!("MEXC: subscribing to depth.full for {} contracts via WebSocket", symbols.len());

            loop {
                info!("Connecting to MEXC Contract WebSocket...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to MEXC Contract WebSocket.");
                        let (mut write, mut read) = ws_stream.split();

                        for chunk in symbols.chunks(sub_batch_size) {
                            for sym in chunk {
                                let sub_msg = json!({
                                    "method": "sub.depth.full",
                                    "param": { "symbol": sym, "limit": 5 }
                                });
                                if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                                    error!("MEXC: failed to subscribe {}: {}", sym, e);
                                    break;
                                }
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }

                        let mut write_ping = write;
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(ping_interval_secs)).await;
                                let ping = json!({"method": "ping"});
                                if write_ping.send(Message::Text(ping.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        });

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let channel = raw.get("channel").and_then(|c| c.as_str()).unwrap_or("");
                                    if channel == "pong" || channel == "rs.sub.depth.full" || raw.get("channel").is_none() {
                                        continue;
                                    }
                                    if channel != "push.depth.full" { continue; }
                                    if let Some(ticker) = parse_mexc_depth_push(&raw) {
                                        if tx_clone.send(ticker).await.is_err() {
                                            error!("MEXC: channel closed");
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("MEXC connection error: {}", e);
                        reconnect_attempt += 1;
                    }
                }
                let delay = super::reconnect_delay_secs(reconnect_attempt);
                info!("MEXC: reconnecting in {}s...", delay);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
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

async fn fetch_mexc_futures_contracts(
    client: &reqwest::Client,
    rest_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/api/v1/contract/detail", rest_url);
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

fn mexc_value_to_decimal(v: &serde_json::Value) -> Option<Decimal> {
    if let Some(n) = v.as_f64() {
        return Decimal::from_str(&n.to_string()).ok();
    }
    if let Some(s) = v.as_str() {
        return Decimal::from_str(s).ok();
    }
    if let Some(n) = v.as_i64() {
        return Some(Decimal::from(n));
    }
    None
}

fn parse_mexc_depth_push(raw: &serde_json::Value) -> Option<UnifiedTicker> {
    let symbol = raw.get("symbol").and_then(|s| s.as_str())?;
    let data = raw.get("data")?;
    let ts = raw.get("ts").and_then(|t| t.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp_millis());

    let bids_raw = data.get("bids").and_then(|b| b.as_array())?;
    let asks_raw = data.get("asks").and_then(|a| a.as_array())?;

    let mut bids_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(bids_raw.len());
    for b in bids_raw.iter().take(5) {
        if let Some(arr) = b.as_array() {
            if arr.len() >= 2 {
                if let (Some(p), Some(q)) = (mexc_value_to_decimal(&arr[0]), mexc_value_to_decimal(&arr[1])) {
                    if p > Decimal::ZERO && q > Decimal::ZERO {
                        bids_vec.push((p, q));
                    }
                }
            }
        }
    }

    let mut asks_vec: Vec<(Decimal, Decimal)> = Vec::with_capacity(asks_raw.len());
    for a in asks_raw.iter().take(5) {
        if let Some(arr) = a.as_array() {
            if arr.len() >= 2 {
                if let (Some(p), Some(q)) = (mexc_value_to_decimal(&arr[0]), mexc_value_to_decimal(&arr[1])) {
                    if p > Decimal::ZERO && q > Decimal::ZERO {
                        asks_vec.push((p, q));
                    }
                }
            }
        }
    }

    if bids_vec.is_empty() || asks_vec.is_empty() {
        return None;
    }

    Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(symbol),
        exchange: ExchangeId::MEXC,
        timestamp: ts,
        bids: bids_vec,
        asks: asks_vec,
    })
}
