use super::Exchange;
use crate::config::ExchangeEndpointConfig;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use rust_decimal::Decimal;
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct GateLauncher {
    ws_url: String,
    rest_url: String,
    max_symbols: usize,
    sub_batch_size: usize,
    sub_batch_delay_ms: u64,
    ping_interval_secs: u64,
}

impl GateLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            ws_url: ep.ws_url.clone().unwrap_or_else(|| "wss://fx-ws.gateio.ws/v4/ws/usdt".into()),
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://fx-api.gateio.ws/api/v4".into()),
            max_symbols: ep.max_symbols.unwrap_or(120),
            sub_batch_size: ep.sub_batch_size.unwrap_or(20),
            sub_batch_delay_ms: ep.sub_batch_delay_ms.unwrap_or(100),
            ping_interval_secs: ep.ping_interval_secs.unwrap_or(15),
        }
    }
}

#[async_trait]
impl Exchange for GateLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();
        let ws_url = self.ws_url.clone();
        let rest_url = self.rest_url.clone();
        let max_symbols = self.max_symbols;
        let sub_batch_size = self.sub_batch_size;
        let sub_batch_delay_ms = self.sub_batch_delay_ms;
        let ping_interval_secs = self.ping_interval_secs;

        tokio::spawn(async move {
            let url = Url::parse(&ws_url).expect("Invalid Gate.io WebSocket URL");
            let mut reconnect_attempt: u32 = 0;

            let client = reqwest::Client::new();
            let mut contracts = Vec::new();
            for attempt in 1..=5 {
                info!("Fetching active futures contracts from Gate.io (attempt {})...", attempt);
                match fetch_gate_futures_contracts(&client, &rest_url).await {
                    Ok(list) => { contracts = list; }
                    Err(e) => error!("Gate: failed to fetch futures contracts: {}", e),
                }
                if !contracts.is_empty() { break; }
                if attempt < 5 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            if contracts.len() > max_symbols {
                contracts.truncate(max_symbols);
            }
            info!("Gate: subscribing to {} futures contracts via WebSocket.", contracts.len());

            loop {
                info!("Connecting to Gate.io WebSocket...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to Gate.io WebSocket.");
                        let (write, mut read) = ws_stream.split();
                        let write = Arc::new(Mutex::new(write));

                        for chunk in contracts.chunks(sub_batch_size) {
                            for contract in chunk {
                                let now = chrono::Utc::now().timestamp();
                                let sub_msg = json!({
                                    "time": now,
                                    "channel": "futures.order_book",
                                    "event": "subscribe",
                                    "payload": [contract, "5", "0"]
                                });
                                let mut w = write.lock().await;
                                if let Err(e) = w.send(Message::Text(sub_msg.to_string())).await {
                                    error!("Gate: subscribe send error: {}", e);
                                    break;
                                }
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(sub_batch_delay_ms)).await;
                        }

                        let write_ping = Arc::clone(&write);
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(ping_interval_secs)).await;
                                let now = chrono::Utc::now().timestamp();
                                let ping = json!({ "time": now, "channel": "futures.ping" });
                                let mut w = write_ping.lock().await;
                                if w.send(Message::Text(ping.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        });

                        while let Some(msg) = read.next().await {
                            let text = match msg {
                                Ok(Message::Text(t)) => t,
                                Ok(Message::Binary(b)) => match String::from_utf8(b) {
                                    Ok(s) => s,
                                    Err(_) => continue,
                                },
                                Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
                                Ok(Message::Close(_)) => { info!("Gate: WebSocket closed by server."); break; }
                                Err(e) => { error!("Gate: WS read error: {}", e); break; }
                                _ => continue,
                            };

                            let raw: serde_json::Value = match serde_json::from_str(&text) {
                                Ok(v) => v,
                                Err(_) => continue,
                            };

                            let channel = raw.get("channel").and_then(|c| c.as_str()).unwrap_or("");
                            if channel == "futures.pong" { continue; }
                            let event = raw.get("event").and_then(|e| e.as_str()).unwrap_or("");
                            if event == "subscribe" { continue; }
                            if channel != "futures.order_book" { continue; }
                            if event != "all" && event != "update" { continue; }

                            let result = match raw.get("result") {
                                Some(r) => r,
                                None => continue,
                            };

                            let contract = result.get("contract").and_then(|c| c.as_str()).unwrap_or("");
                            if contract.is_empty() { continue; }

                            let ts = result.get("t").and_then(|t| t.as_i64())
                                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
                            let bids_vec = parse_gate_levels(result.get("bids"));
                            let asks_vec = parse_gate_levels(result.get("asks"));
                            if bids_vec.is_empty() || asks_vec.is_empty() { continue; }

                            let ticker = UnifiedTicker {
                                symbol: crate::model::normalize_symbol(contract),
                                exchange: ExchangeId::Gate,
                                timestamp: ts,
                                bids: bids_vec,
                                asks: asks_vec,
                            };
                            if tx_clone.send(ticker).await.is_err() {
                                error!("Gate: channel closed");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Gate: WebSocket connection error: {}", e);
                        reconnect_attempt += 1;
                    }
                }
                let delay = super::reconnect_delay_secs(reconnect_attempt);
                info!("Gate: reconnecting in {}s...", delay);
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

async fn fetch_gate_futures_contracts(
    client: &reqwest::Client,
    rest_url: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/futures/usdt/contracts", rest_url);
    let list: Vec<serde_json::Value> = client.get(&url).send().await?.json().await?;
    let symbols: Vec<String> = list
        .iter()
        .filter_map(|item| {
            let name = item.get("name").and_then(|n| n.as_str())?;
            if name.ends_with("_USDT") { Some(name.to_string()) } else { None }
        })
        .collect();
    Ok(symbols)
}

fn parse_gate_levels(val: Option<&serde_json::Value>) -> Vec<(Decimal, Decimal)> {
    let arr = match val.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut levels = Vec::with_capacity(arr.len());
    for item in arr.iter().take(5) {
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        let price = obj.get("p").and_then(|v| v.as_str()).and_then(|s| Decimal::from_str(s).ok());
        let size = obj.get("s").and_then(gate_value_to_decimal);
        if let (Some(p), Some(s)) = (price, size) {
            if p > Decimal::ZERO && s > Decimal::ZERO { levels.push((p, s)); }
        }
    }
    levels
}

fn gate_value_to_decimal(v: &serde_json::Value) -> Option<Decimal> {
    if let Some(n) = v.as_i64() { return Some(Decimal::from(n)); }
    if let Some(n) = v.as_u64() { return Some(Decimal::from(n)); }
    if let Some(s) = v.as_str() { return Decimal::from_str(s).ok(); }
    if let Some(n) = v.as_f64() { return Decimal::from_str(&n.to_string()).ok(); }
    None
}
