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

pub struct BybitLauncher {
    ws_url: String,
    rest_url: String,
    max_symbols: usize,
    sub_batch_size: usize,
    sub_batch_delay_ms: u64,
}

impl BybitLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            ws_url: ep.ws_url.clone().unwrap_or_else(|| "wss://stream.bybit.com/v5/public/linear".into()),
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://api.bybit.com".into()),
            max_symbols: ep.max_symbols.unwrap_or(120),
            sub_batch_size: ep.sub_batch_size.unwrap_or(50),
            sub_batch_delay_ms: ep.sub_batch_delay_ms.unwrap_or(100),
        }
    }
}

#[async_trait]
impl Exchange for BybitLauncher {
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

        tokio::spawn(async move {
            let url = Url::parse(&ws_url).expect("Invalid Bybit WebSocket URL");
            let mut reconnect_attempt: u32 = 0;
            let client = reqwest::Client::new();
            let mut symbols = Vec::new();
            for attempt in 1..=5 {
                info!("Fetching active trading pairs from Bybit API (attempt {})...", attempt);
                let mut cursor = String::new();
                symbols.clear();
                loop {
                    let fetch_url = if cursor.is_empty() {
                        format!("{}/v5/market/instruments-info?category=linear&limit=1000&status=Trading", rest_url)
                    } else {
                        format!("{}/v5/market/instruments-info?category=linear&limit=1000&status=Trading&cursor={}", rest_url, cursor)
                    };
                    match client.get(&fetch_url).send().await {
                        Ok(resp) => {
                            if let Ok(json) = resp.json::<serde_json::Value>().await {
                                if let Some(list) = json["result"]["list"].as_array() {
                                    for item in list {
                                        if let Some(s) = item["symbol"].as_str() {
                                            if s.ends_with("USDT") {
                                                symbols.push(crate::model::normalize_symbol(s));
                                            }
                                        }
                                    }
                                }
                                if let Some(next) = json["result"]["nextPageCursor"].as_str() {
                                    if !next.is_empty() {
                                        cursor = next.to_string();
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(e) => error!("Failed to fetch Bybit pairs: {}", e),
                    }
                    break;
                }
                if !symbols.is_empty() { break; }
                if attempt < 5 {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
            info!("Bybit: Discovered {} active USDT pairs.", symbols.len());
            if symbols.len() > max_symbols {
                symbols.truncate(max_symbols);
            }

            loop {
                info!("Connecting to Bybit Linear...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to Bybit.");
                        let (mut write, mut read) = ws_stream.split();

                        for chunk in symbols.chunks(sub_batch_size) {
                            let args: Vec<String> = chunk.iter().map(|s| format!("orderbook.50.{}", s)).collect();
                            let sub_msg = json!({ "op": "subscribe", "args": args });
                            if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                                error!("Failed to subscribe Bybit batch: {}", e);
                                break;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_millis(sub_batch_delay_ms)).await;
                        }

                        let mut write_ping = write;
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                                let ping = json!({"op": "ping"});
                                if write_ping.send(Message::Text(ping.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        });

                        let mut bybit_logged_unparseable = false;
                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if raw.get("success").and_then(|s| s.as_bool()) == Some(true) {
                                        continue;
                                    }
                                    let topic = raw.get("topic").and_then(|t| t.as_str()).unwrap_or("");
                                    let data = match raw.get("data") {
                                        Some(d) => d,
                                        None => continue,
                                    };
                                    let sym = if topic.starts_with("orderbook.50.") {
                                        crate::model::normalize_symbol(&topic.replace("orderbook.50.", ""))
                                    } else {
                                        crate::model::normalize_symbol(data.get("s").and_then(|s| s.as_str()).unwrap_or(""))
                                    };
                                    let mut bids_vec = Vec::new();
                                    let mut asks_vec = Vec::new();
                                    if let Some(bids) = data.get("b").and_then(|x| x.as_array()) {
                                        for b in bids.iter().take(5) {
                                            if let Some(arr) = b.as_array() {
                                                if arr.len() >= 2 {
                                                    if let (Some(p), Some(q)) = (json_value_to_decimal(&arr[0]), json_value_to_decimal(&arr[1])) {
                                                        bids_vec.push((p, q));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if let Some(asks) = data.get("a").and_then(|x| x.as_array()) {
                                        for a in asks.iter().take(5) {
                                            if let Some(arr) = a.as_array() {
                                                if arr.len() >= 2 {
                                                    if let (Some(p), Some(q)) = (json_value_to_decimal(&arr[0]), json_value_to_decimal(&arr[1])) {
                                                        asks_vec.push((p, q));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if !bids_vec.is_empty() && !asks_vec.is_empty() && !sym.is_empty() {
                                        let ticker = UnifiedTicker {
                                            symbol: sym,
                                            exchange: ExchangeId::Bybit,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                            bids: bids_vec,
                                            asks: asks_vec,
                                        };
                                        let _ = tx_clone.send(ticker).await;
                                    }
                                } else if !bybit_logged_unparseable && text.len() > 10 {
                                    bybit_logged_unparseable = true;
                                    let preview = if text.len() > 250 { format!("{}...", &text[..250]) } else { text.clone() };
                                    info!("Bybit WS sample (unparseable): {}", preview.replace('\n', " "));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Bybit connection error: {}", e);
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

fn json_value_to_decimal(v: &serde_json::Value) -> Option<Decimal> {
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
