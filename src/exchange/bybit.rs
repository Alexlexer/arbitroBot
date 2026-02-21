use super::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use log::{error, info};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use std::str::FromStr;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;

pub struct BybitLauncher;

#[async_trait]
impl Exchange for BybitLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();
        
        tokio::spawn(async move {
            let url = Url::parse("wss://stream.bybit.com/v5/public/linear").unwrap();
            // Dynamic Fetch
            info!("Fetching active trading pairs from Bybit API...");
            
            let client = reqwest::Client::new();
            let mut symbols = Vec::new();
            let mut cursor = String::new();

            loop {
                // Fetch up to 1000 active linear pairs
                let url = if cursor.is_empty() {
                    "https://api.bybit.com/v5/market/instruments-info?category=linear&limit=1000&status=Trading".to_string()
                } else {
                    format!("https://api.bybit.com/v5/market/instruments-info?category=linear&limit=1000&status=Trading&cursor={}", cursor)
                };

                match client.get(&url).send().await {
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
                             // Pagination check
                             if let Some(next) = json["result"]["nextPageCursor"].as_str() {
                                 if !next.is_empty() {
                                     cursor = next.to_string();
                                     continue;
                                 }
                             }
                         }
                    }
                    Err(e) => {
                        error!("Failed to fetch Bybit pairs: {}", e);
                         // Fallback to minimal list if API fails
                        symbols = vec!["BTCUSDT".into(), "ETHUSDT".into()];
                    }
                }
                break;
            }
            
            info!("Bybit: Discovered {} active USDT pairs.", symbols.len());
            // Filter to top 100 or so to avoid overwhelming single connection?
            // "Trash coins" implies we want many. AWS t2.micro might handle 200-300? 
            // Let's take first 300 for now.
            if symbols.len() > 300 {
                symbols.truncate(300);
            }


            loop {
                info!("Connecting to Bybit Linear...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        info!("Connected to Bybit.");
                        let (mut write, mut read) = ws_stream.split();

                        // Subscribe
                        let args: Vec<String> = symbols.iter().map(|s| format!("orderbook.50.{}", s)).collect();
                        let sub_msg = json!({
                            "op": "subscribe",
                            "args": args
                        });
                        
                        if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                            error!("Failed to subscribe Bybit: {}", e);
                            continue;
                        }

                        // Ping loop
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

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(event) = serde_json::from_str::<BybitResponse>(&text) {
                                    if let Some(data) = event.data {
                                        let sym = crate::model::normalize_symbol(&event.topic.unwrap_or_default().replace("orderbook.50.", ""));
                                        
                                        let mut bids_vec = Vec::new();
                                        let mut asks_vec = Vec::new();

                                        if let Some(bids) = data.b {
                                            for b in bids.iter().take(5) {
                                                if let (Ok(p), Ok(q)) = (Decimal::from_str(&b[0]), Decimal::from_str(&b[1])) {
                                                    bids_vec.push((p, q));
                                                }
                                            }
                                        }
                                        if let Some(asks) = data.a {
                                            for a in asks.iter().take(5) {
                                                if let (Ok(p), Ok(q)) = (Decimal::from_str(&a[0]), Decimal::from_str(&a[1])) {
                                                    asks_vec.push((p, q));
                                                }
                                            }
                                        }

                                        if !bids_vec.is_empty() && !asks_vec.is_empty() {
                                            let ticker = UnifiedTicker {
                                                symbol: sym,
                                                exchange: ExchangeId::Bybit,
                                                timestamp: chrono::Utc::now().timestamp_millis(),
                                                bids: bids_vec,
                                                asks: asks_vec,
                                            };
                                            let _ = tx_clone.send(ticker).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Bybit connection error: {}", e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
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

#[derive(Deserialize)]
struct BybitResponse {
    topic: Option<String>,
    data: Option<BybitData>,
}

#[derive(Deserialize)]
struct BybitData {
    b: Option<Vec<Vec<String>>>, // Bids [[price, size]]
    a: Option<Vec<Vec<String>>>, // Asks
}
