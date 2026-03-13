use async_trait::async_trait;
use crate::exchange::Exchange;
use crate::model::{ExchangeId, UnifiedTicker};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use log::{info, error};
use std::str::FromStr;

pub struct MexcLauncher;

#[async_trait]
impl Exchange for MexcLauncher {
    async fn connect(&mut self, tx: Sender<UnifiedTicker>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to MEXC Market Data (REST All Tickers)...");
        
        let client = reqwest::Client::new();
        let tx_clone = tx.clone();
        
        tokio::spawn(async move {
            loop {
                // Fetch all tickers from MEXC
                match client.get("https://api.mexc.com/api/v3/ticker/bookTicker").send().await {
                    Ok(resp) => {
                        if let Ok(tickers) = resp.json::<Vec<serde_json::Value>>().await {
                            for t in tickers {
                                if let (Some(s), Some(b), Some(a)) = (t["symbol"].as_str(), t["bidPrice"].as_str(), t["askPrice"].as_str()) {
                                    // Filter for USDT pairs
                                    if s.ends_with("USDT") || s.ends_with("_USDT") {
                                        let symbol = crate::model::normalize_symbol(s);
                                        let bid = Decimal::from_str(b).unwrap_or(Decimal::ZERO);
                                        let ask = Decimal::from_str(a).unwrap_or(Decimal::ZERO);
                                        
                                        if bid > Decimal::ZERO && ask > Decimal::ZERO {
                                            let ut = UnifiedTicker {
                                                exchange: ExchangeId::MEXC,
                                                symbol,
                                                bids: vec![(bid, Decimal::from(100))], // Size placeholder
                                                asks: vec![(ask, Decimal::from(100))],
                                                timestamp: chrono::Utc::now().timestamp_millis(),
                                            };
                                            let _ = tx_clone.send(ut).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => error!("MEXC Poll Error: {}", e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            }
        });
        
        Ok(())
    }

    async fn subscribe(&mut self, _symbols: &[String]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
