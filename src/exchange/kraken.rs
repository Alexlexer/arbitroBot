use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};

pub struct KrakenLauncher;

#[async_trait]
impl Exchange for KrakenLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Kraken Market Data (REST All Tickers)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match client.get("https://api.kraken.com/0/public/Ticker").send().await {
                    Ok(resp) => {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(result) = json["result"].as_object() {
                                for (s, t) in result {
                                    if s.ends_with("USD") || s.ends_with("USDT") {
                                        let symbol = crate::model::normalize_symbol(s);
                                        if let (Some(b), Some(a)) = (t["b"][0].as_str(), t["a"][0].as_str()) {
                                            let bid = b.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                            let ask = a.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                            if bid > Decimal::ZERO {
                                                let _ = tx_clone.send(UnifiedTicker {
                                                    exchange: ExchangeId::Kraken,
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
                    Err(e) => error!("Kraken Poll Error: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}
