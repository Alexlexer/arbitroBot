use crate::model::{ArbitrageOpportunity, ExchangeId};
use crate::rate_limiter::RateLimiter;
use log::{info, error};
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::time::{sleep, Duration};

pub struct ExecutionActor {
    rx: Receiver<ArbitrageOpportunity>,
    rate_limiter: Arc<RateLimiter>,
}

impl ExecutionActor {
    pub fn new(rx: Receiver<ArbitrageOpportunity>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self { rx, rate_limiter }
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

        // Check Rate Limits Before Multi-Leg Execution
        if !self.rate_limiter.check_limit(opp.long_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Order Rate Limit Exceeded", opp.long_exchange);
            return;
        }
        if !self.rate_limiter.check_limit(opp.short_exchange, true, 1.0).await {
            error!("Execution Aborted: {} Order Rate Limit Exceeded", opp.short_exchange);
            return;
        }

        // Mock success for now
        let _ = self.place_order(opp.long_exchange, &opp.symbol, "BUY", opp.long_price).await;
        let _ = self.place_order(opp.short_exchange, &opp.symbol, "SELL", opp.short_price).await;

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
