use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};

pub struct BitmartLauncher;

#[async_trait]
impl Exchange for BitmartLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Bitmart Market Data (REST All Tickers)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match client.get("https://api-cloud.bitmart.com/spot/v1/ticker").send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(tickers) = json["data"]["tickers"].as_array() {
                                for t in tickers {
                                    if let (Some(s), Some(b), Some(a)) = (t["symbol"].as_str(), t["bid_px"].as_str(), t["ask_px"].as_str()) {
                                        if s.ends_with("_USDT") {
                                            let symbol = crate::model::normalize_symbol(s);
                                            let bid = b.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                            let ask = a.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                            if bid > Decimal::ZERO {
                                                let _ = tx_clone.send(UnifiedTicker {
                                                    exchange: ExchangeId::Bitmart,
                                                    symbol,
                                                    bids: vec![(bid, Decimal::from(100))],
                                                    asks: vec![(ask, Decimal::from(100))],
                                                    timestamp: chrono::Utc::now().timestamp_millis(),
                                                }).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => error!("Bitmart Poll Error: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}
