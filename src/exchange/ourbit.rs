use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};

pub struct OurbitLauncher;

#[async_trait]
impl Exchange for OurbitLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Ourbit Market Data (REST All Tickers)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                // Ourbit often uses MEXC-like API
                match client.get("https://api.ourbit.com/api/v1/ticker/24hr").send().await {
                    Ok(resp) => {
                        if let Ok(tickers) = resp.json::<Vec<serde_json::Value>>().await {
                            for t in tickers {
                                if let (Some(s), Some(b), Some(a)) = (t["symbol"].as_str(), t["bidPrice"].as_str(), t["askPrice"].as_str()) {
                                    if s.ends_with("_USDT") {
                                        let symbol = crate::model::normalize_symbol(s);
                                        let bid = b.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                        let ask = a.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                        if bid > Decimal::ZERO {
                                            let _ = tx_clone.send(UnifiedTicker {
                                                exchange: ExchangeId::Ourbit,
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
                    Err(e) => error!("Ourbit Poll Error: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(12)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}
