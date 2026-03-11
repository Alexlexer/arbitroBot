use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};

pub struct GateLauncher;

#[async_trait]
impl Exchange for GateLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to Gate.io Market Data (REST All Tickers)...");
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            loop {
                match client.get("https://api.gateio.ws/api/v4/spot/tickers").send().await {
                    Ok(resp) => {
                        if let Ok(tickers) = resp.json::<Vec<serde_json::Value>>().await {
                            for t in tickers {
                                if let (Some(s), Some(b), Some(a)) = (t["currency_pair"].as_str(), t["lowest_ask"].as_str(), t["highest_bid"].as_str()) {
                                    if s.ends_with("_USDT") {
                                        let symbol = crate::model::normalize_symbol(s);
                                        let bid = a.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                        let ask = b.parse::<Decimal>().unwrap_or(Decimal::ZERO);
                                        if bid > Decimal::ZERO {
                                            let _ = tx_clone.send(UnifiedTicker {
                                                exchange: ExchangeId::Gate,
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
                    Err(e) => error!("Gate Poll Error: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
        Ok(())
    }
    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> { Ok(()) }
}
