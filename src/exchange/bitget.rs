use super::Exchange;
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

pub struct BitgetLauncher;

#[async_trait]
impl Exchange for BitgetLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let url = Url::parse("wss://ws.bitget.com/v2/ws/public")
                .expect("Invalid Bitget WebSocket URL");
            let mut reconnect_attempt: u32 = 0;
            // Dynamic Fetch
            let client = reqwest::Client::new();
            let mut symbols = Vec::new();
            for attempt in 1..=5 {
                info!("Fetching active trading pairs from Bitget API (attempt {})...", attempt);
                symbols.clear();
                match client.get("https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES").send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                                for d in data {
                                    let inst_id = d.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                                    if inst_id.ends_with("USDT") {
                                        symbols.push(crate::model::normalize_symbol(inst_id));
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => error!("Failed to fetch Bitget pairs: {}", e),
                }
                if !symbols.is_empty() {
                    break;
                }
                if attempt < 5 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            info!("Bitget: Discovered {} active USDT pairs.", symbols.len());
            if symbols.len() > 120 {
                symbols.truncate(120);
            }

            loop {
                info!("Connecting to Bitget...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to Bitget.");
                        let (mut write, mut read) = ws_stream.split();

                        // Subscribe in batches of 40
                        for chunk in symbols.chunks(40) {
                            let args: Vec<_> = chunk.iter().map(|s| {
                                json!({
                                    "instType": "USDT-FUTURES",
                                    "channel": "books5",
                                    "instId": s
                                })
                            }).collect();
                            let sub_msg = json!({ "op": "subscribe", "args": args });
                            if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                                error!("Failed to subscribe Bitget batch: {}", e);
                                break;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                        }

                         // Ping loop
                        let mut write_ping = write;
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(25)).await;
                                if write_ping.send(Message::Text("ping".to_string())).await.is_err() {
                                    break;
                                }
                            }
                        });


                        let mut bitget_logged_unparseable = false;
                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if text == "pong" || text.starts_with("{\"event\":\"pong\"") { continue; }
                                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if raw.get("event").and_then(|e| e.as_str()) == Some("subscribe") {
                                        continue;
                                    }
                                    let inst_id = raw.get("arg").and_then(|a| a.get("instId")).and_then(|v| v.as_str()).unwrap_or("");
                                    let mut books: Vec<&serde_json::Value> = Vec::new();
                                    if let Some(data) = raw.get("data").and_then(|d| d.as_array()) {
                                        for item in data.iter() {
                                            if let Some(inner) = item.as_array() {
                                                for book in inner.iter() {
                                                    if book.is_object() {
                                                        books.push(book);
                                                    }
                                                }
                                            } else if item.is_object() {
                                                books.push(item);
                                            }
                                        }
                                    }
                                    for book in books {
                                        let obj = match book.as_object() {
                                            Some(o) => o,
                                            None => continue,
                                        };
                                        let mut bids_vec = Vec::new();
                                        let mut asks_vec = Vec::new();
                                        if let Some(bids) = obj.get("bids").and_then(|x| x.as_array()) {
                                            for b in bids.iter().take(5) {
                                                if let Some(arr) = b.as_array() {
                                                    if arr.len() >= 2 {
                                                        if let (Some(p), Some(q)) = (bitget_value_to_decimal(&arr[0]), bitget_value_to_decimal(&arr[1])) {
                                                            bids_vec.push((p, q));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        if let Some(asks) = obj.get("asks").and_then(|x| x.as_array()) {
                                            for a in asks.iter().take(5) {
                                                if let Some(arr) = a.as_array() {
                                                    if arr.len() >= 2 {
                                                        if let (Some(p), Some(q)) = (bitget_value_to_decimal(&arr[0]), bitget_value_to_decimal(&arr[1])) {
                                                            asks_vec.push((p, q));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        let sym = obj.get("instId").and_then(|v| v.as_str()).unwrap_or(inst_id);
                                        if !bids_vec.is_empty() && !asks_vec.is_empty() && !sym.is_empty() {
                                            let ticker = UnifiedTicker {
                                                symbol: crate::model::normalize_symbol(sym),
                                                exchange: ExchangeId::Bitget,
                                                timestamp: chrono::Utc::now().timestamp_millis(),
                                                bids: bids_vec,
                                                asks: asks_vec,
                                            };
                                            let _ = tx_clone.send(ticker).await;
                                        }
                                    }
                                } else if !bitget_logged_unparseable && text.len() > 10 {
                                    bitget_logged_unparseable = true;
                                    let preview = if text.len() > 250 { format!("{}...", &text[..250]) } else { text.clone() };
                                    info!("Bitget WS sample (unparseable): {}", preview.replace('\n', " "));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Bitget connection error: {}", e);
                        reconnect_attempt += 1;
                    }
                }
                let delay = super::reconnect_delay_secs(reconnect_attempt);
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

fn bitget_value_to_decimal(v: &serde_json::Value) -> Option<Decimal> {
    if let Some(s) = v.as_str() {
        return Decimal::from_str(s).ok();
    }
    if let Some(n) = v.as_f64() {
        return Decimal::from_str(&n.to_string()).ok();
    }
    if let Some(n) = v.as_i64() {
        return Some(Decimal::from(n));
    }
    None
}
