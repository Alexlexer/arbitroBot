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
            info!("Fetching active trading pairs from Bitget API...");
            let client = reqwest::Client::new();
            let mut symbols = Vec::new();

            match client.get("https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES")
                .send().await 
            {
                Ok(resp) => {
                     if let Ok(json) = resp.json::<serde_json::Value>().await {
                         // Bitget response: { code: "00000", data: [ { symbol: "BTCUSDT", ... } ] }
                         if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            for d in data {
                                let inst_id = d.get("instId").and_then(|v| v.as_str()).unwrap_or("");
                                // let timestamp = d.get("ts").and_then(|v| v.as_str()).unwrap_or("0").parse::<i64>().unwrap_or(0); // This line was in the snippet but not used for symbol collection
                                if inst_id.ends_with("USDT") {
                                    symbols.push(crate::model::normalize_symbol(inst_id));
                                }
                            }
                         }
                     }
                }
                Err(e) => {
                    error!("Failed to fetch Bitget pairs: {}", e);
                    symbols = vec!["BTCUSDT".into(), "ETHUSDT".into()];
                }
            }
            
            info!("Bitget: Discovered {} active USDT pairs.", symbols.len());
            if symbols.len() > 300 {
                symbols.truncate(300);
            }

            loop {
                info!("Connecting to Bitget...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Connected to Bitget.");
                        let (mut write, mut read) = ws_stream.split();

                        // Subscribe
                        let args: Vec<_> = symbols.iter().map(|s| {
                            json!({
                                "instType": "USDT-FUTURES",
                                "channel": "books5",
                                "instId": s
                            })
                        }).collect();

                        let sub_msg = json!({
                            "op": "subscribe",
                            "args": args
                        });

                         if let Err(e) = write.send(Message::Text(sub_msg.to_string())).await {
                            error!("Failed to subscribe Bitget: {}", e);
                            continue;
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


                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if text == "pong" { continue; }
                                
                                if let Ok(event) = serde_json::from_str::<BitgetResponse>(&text) {
                                    if let Some(data) = event.data {
                                        for d in data {
                                              let mut bids_vec = Vec::new();
                                              let mut asks_vec = Vec::new();

                                              for b in d.bids.iter().take(5) {
                                                  if let (Ok(p), Ok(q)) = (Decimal::from_str(&b[0]), Decimal::from_str(&b[1])) {
                                                      bids_vec.push((p, q));
                                                  }
                                              }
                                              for a in d.asks.iter().take(5) {
                                                  if let (Ok(p), Ok(q)) = (Decimal::from_str(&a[0]), Decimal::from_str(&a[1])) {
                                                      asks_vec.push((p, q));
                                                  }
                                              }

                                              if !bids_vec.is_empty() && !asks_vec.is_empty() {
                                                  let ticker = UnifiedTicker {
                                                      symbol: crate::model::normalize_symbol(&d.inst_id),
                                                      exchange: ExchangeId::Bitget,
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

#[derive(Deserialize)]
struct BitgetResponse {
    data: Option<Vec<BitgetData>>,
}

#[derive(Deserialize)]
struct BitgetData {
    #[serde(rename = "instId")]
    inst_id: String,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}
