use crate::model::{ArbitrageOpportunity, ExchangeId};
use log::{info, warn};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Receiver;
use tokio::time::{sleep, Duration};

pub struct ExecutionActor {
    rx: Receiver<ArbitrageOpportunity>,
}

impl ExecutionActor {
    pub fn new(rx: Receiver<ArbitrageOpportunity>) -> Self {
        Self { rx }
    }

    pub async fn run(&mut self) {
        info!("Execution Actor started.");

        while let Some(opp) = self.rx.recv().await {
            self.execute_opportunity(opp).await;
        }
    }

    async fn execute_opportunity(&self, opp: ArbitrageOpportunity) {
        info!(
            "👀 OPPORTUNITY FOUND: Long {} on {}, Short {} on {} | Net Spread: {:.2}%",
            opp.symbol, opp.long_exchange, opp.symbol, opp.short_exchange, opp.spread_pct
        );

        // EXECUTION LOGIC with Latency Guard
        /*
        let (long_res, short_res) = tokio::join!(
            self.place_order(opp.long_exchange, &opp.symbol, "BUY", opp.long_price),
            self.place_order(opp.short_exchange, &opp.symbol, "SELL", opp.short_price)
        );

        match (long_res, short_res) {
            (Ok(_), Ok(_)) => info!("✅ ARBITRAGE SUCCESS: Both legs executed."),
            (Ok(_), Err(e)) => {
                error!("⚠️ PARTIAL FILL (Long OK, Short FAIL): {}. Closing Long leg immediately...", e);
                let _ = self.place_order(opp.long_exchange, &opp.symbol, "SELL", opp.long_price).await;
            }
            (Err(e), Ok(_)) => {
                error!("⚠️ PARTIAL FILL (Long FAIL, Short OK): {}. Closing Short leg immediately...", e);
                let _ = self.place_order(opp.short_exchange, &opp.symbol, "BUY", opp.short_price).await;
            }
            (Err(e1), Err(e2)) => error!("❌ BOTH LEGS FAILED: {} | {}", e1, e2),
        }
        */

        info!("(Execution Disabled) - Simulated Latency Guard: OK");
    }

    async fn place_order(&self, exchange: ExchangeId, symbol: &str, side: &str, price: Decimal) -> Result<(), String> {
        // Mock network delay
        sleep(Duration::from_millis(50)).await;
        
        info!(
            "   -> Order on {:?}: {} {} @ {}",
            exchange, side, symbol, price
        );
        
        // Mock success rate
        Ok(()) 
    }
}
