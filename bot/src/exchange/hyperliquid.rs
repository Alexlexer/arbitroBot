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

pub struct HyperliquidLauncher {
    ws_url: String,
    rest_url: String,
    max_symbols: usize,
    ping_interval_secs: u64,
}

impl HyperliquidLauncher {
    pub fn new(ep: &ExchangeEndpointConfig) -> Self {
        Self {
            ws_url: ep.ws_url.clone().unwrap_or_else(|| "wss://api.hyperliquid.xyz/ws".into()),
            rest_url: ep.rest_url.clone().unwrap_or_else(|| "https://api.hyperliquid.xyz".into()),
            max_symbols: ep.max_symbols.unwrap_or(100),
            ping_interval_secs: ep.ping_interval_secs.unwrap_or(30),
        }
    }
}

#[async_trait]
impl Exchange for HyperliquidLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tx_clone = tx.clone();
        let ws_url = self.ws_url.clone();
        let rest_url = self.rest_url.clone();
        let max_symbols = self.max_symbols;
        let ping_interval_secs = self.ping_interval_secs;

        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut coins: Vec<String> = Vec::new();

            for attempt in 1..=5 {
                info!("Hyperliquid: fetching perpetuals metadata (attempt {})...", attempt);
                let meta_url = format!("{}/info", rest_url);
                match client.post(&meta_url).json(&json!({"type": "meta"})).send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(universe) = json["universe"].as_array() {
                                coins = universe
                                    .iter()
                                    .filter_map(|v| v["name"].as_str().map(str::to_owned))
                                    .take(max_symbols)
                                    .collect();
                            }
                        }
                    }
                    Err(e) => error!("Hyperliquid: meta fetch error: {}", e),
                }
                if !coins.is_empty() { break; }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }

            info!("Hyperliquid: {} perpetuals discovered.", coins.len());

            let url = Url::parse(&ws_url).expect("Invalid Hyperliquid WS URL");
            let mut reconnect_attempt: u32 = 0;

            loop {
                info!("Hyperliquid: connecting to WebSocket...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0;
                        info!("Hyperliquid: connected.");
                        let (mut write, mut read) = ws_stream.split();

                        for coin in &coins {
                            let sub = json!({
                                "method": "subscribe",
                                "subscription": {"type": "l2Book", "coin": coin}
                            });
                            if let Err(e) = write.send(Message::Text(sub.to_string())).await {
                                error!("Hyperliquid: subscribe error for {}: {}", coin, e);
                                break;
                            }
                        }

                        let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<()>(1);
                        tokio::spawn(async move {
                            loop {
                                tokio::time::sleep(tokio::time::Duration::from_secs(ping_interval_secs)).await;
                                if ping_tx.send(()).await.is_err() { break; }
                            }
                        });

                        let mut logged_unparseable = false;
                        loop {
                            tokio::select! {
                                msg = read.next() => {
                                    match msg {
                                        Some(Ok(Message::Text(text))) => {
                                            if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) {
                                                if raw.get("channel").and_then(|c| c.as_str()) == Some("l2Book") {
                                                    if let Some(ticker) = parse_l2book(&raw) {
                                                        let _ = tx_clone.send(ticker).await;
                                                    }
                                                }
                                            } else if !logged_unparseable {
                                                logged_unparseable = true;
                                                let preview = if text.len() > 200 { format!("{}...", &text[..200]) } else { text.clone() };
                                                info!("Hyperliquid WS sample: {}", preview.replace('\n', " "));
                                            }
                                        }
                                        Some(Ok(_)) => {}
                                        Some(Err(e)) => { error!("Hyperliquid WS error: {}", e); break; }
                                        None => break,
                                    }
                                }
                                Some(_) = ping_rx.recv() => {
                                    let ping = json!({"method": "ping"});
                                    if write.send(Message::Text(ping.to_string())).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Hyperliquid: connection error: {}", e);
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

fn parse_l2book(raw: &serde_json::Value) -> Option<UnifiedTicker> {
    let data = raw.get("data")?;
    let coin = data.get("coin")?.as_str()?;
    let levels = data.get("levels")?.as_array()?;
    if levels.len() < 2 { return None; }

    let parse_levels = |arr: &serde_json::Value| -> Vec<(Decimal, Decimal)> {
        arr.as_array()
            .map(|entries| {
                entries.iter().take(5).filter_map(|entry| {
                    let row = entry.as_array()?;
                    if row.len() < 2 { return None; }
                    let price = Decimal::from_str(row[0].as_str()?).ok()?;
                    let size = Decimal::from_str(row[1].as_str()?).ok()?;
                    if price > Decimal::ZERO && size > Decimal::ZERO { Some((price, size)) } else { None }
                }).collect()
            })
            .unwrap_or_default()
    };

    let bids = parse_levels(&levels[0]);
    let asks = parse_levels(&levels[1]);
    if bids.is_empty() || asks.is_empty() { return None; }

    Some(UnifiedTicker {
        symbol: crate::model::normalize_symbol(coin),
        exchange: ExchangeId::Hyperliquid,
        timestamp: data.get("time").and_then(|t| t.as_i64()).unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
        bids,
        asks,
    })
}
