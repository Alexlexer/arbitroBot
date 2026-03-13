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

#[async_trait]
impl Exchange for BinanceLauncher {
    async fn connect(
        &mut self,
        tx: Sender<UnifiedTicker>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Binance Futures WebSocket URL
        // We will subscribe to all tickers or a subset in 'subscribe'
        // For simpler architecture, we'll spawn the connection loop here.
        
        tokio::spawn(async move {
            let url = Url::parse("wss://fstream.binance.com/ws/!bookTicker")
                .expect("Invalid Binance WebSocket URL");
            let mut reconnect_attempt: u32 = 0;
            
            loop {
                info!("Connecting to Binance Futures...");
                match connect_async(url.clone()).await {
                    Ok((ws_stream, _)) => {
                        reconnect_attempt = 0; // Reset on success
                        info!("Connected to Binance Futures.");
                        let (_, mut read) = ws_stream.split();

                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(text)) = msg {
                                if let Ok(event) = serde_json::from_str::<BookTickerEvent>(&text) {
                                    if let (Ok(bid_p), Ok(bid_q), Ok(ask_p), Ok(ask_q)) = (
                                        Decimal::from_str(&event.b),
                                        Decimal::from_str(&event._bid_qty),
                                        Decimal::from_str(&event.a),
                                        Decimal::from_str(&event._ask_qty),
                                    ) {
                                        let ticker = UnifiedTicker {
                                            symbol: crate::model::normalize_symbol(&event.s),
                                            exchange: ExchangeId::Binance,
                                            timestamp: chrono::Utc::now().timestamp_millis(),
                                            bids: vec![(bid_p, bid_q)],
                                            asks: vec![(ask_p, ask_q)],
                                        };
                                        if let Err(e) = tx.send(ticker).await {
                                            error!("Failed to send Binance ticker: {}", e);
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
        // Since we connected to !bookTicker stream which pushes all pairs, 
        // explicit subscription isn't strictly needed for the MVP if we filter later.
        // However, standard implementation would send a SUBSCRIBE message here.
        Ok(())
    }
}

#[derive(Deserialize)]
struct BookTickerEvent {
    s: String, // Symbol
    b: String, // Best bid price
    #[serde(rename = "B")]
    _bid_qty: String, // Best bid qty
    a: String, // Best ask price
    #[serde(rename = "A")]
    _ask_qty: String, // Best ask qty
}
