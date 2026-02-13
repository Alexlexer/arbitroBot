use crate::model::{
    ExchangeId, ArbitrageOpportunity, OrderBookDepth, FundingInfo, AssetStatus, RiskError
};
use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::collections::HashMap;
use log::{info, warn};

pub struct RiskManager {
    // Configuration constants could go here
}

impl RiskManager {
    pub fn new() -> Self {
        Self {}
    }

    /// Primary validation entry point
    pub async fn validate(
        &self, 
        opp: &ArbitrageOpportunity,
        depth_map: &HashMap<ExchangeId, OrderBookDepth>,
        funding_map: &HashMap<ExchangeId, FundingInfo>,
        status_map: &HashMap<ExchangeId, AssetStatus>,
        target_volume_usdt: Decimal, 
    ) -> Result<(), RiskError> {
        
        // 1. Check Wallet Status
        self.check_wallet_status(opp, status_map)?;

        // 2. Check Liquidity / Slippage
        self.check_liquidity(opp, depth_map, target_volume_usdt)?;

        // 3. Check Funding Rate
        self.check_funding(opp, funding_map)?;

        Ok(())
    }

    /// Liquidity Guard: Ensure top 5 levels have 3x order volume
    fn check_liquidity(
        &self,
        opp: &ArbitrageOpportunity,
        depth_map: &HashMap<ExchangeId, OrderBookDepth>,
        target_volume_usdt: Decimal,
    ) -> Result<(), RiskError> {
        let required_liquidity = target_volume_usdt * Decimal::from(3);

        // Check Long Exchange (Ask side liquidity needed)
        let long_depth = depth_map.get(&opp.long_exchange).ok_or(
            RiskError::LowLiquidity(format!("No depth data for {}", opp.long_exchange))
        )?;
        
        let long_liquidity_sum: Decimal = long_depth.asks.iter().take(5)
            .map(|(p, s)| p * s)
            .sum();

        if long_liquidity_sum < required_liquidity {
            return Err(RiskError::LowLiquidity(format!(
                "{} Asks (Top 5): ${} < Required ${}", 
                opp.long_exchange, long_liquidity_sum, required_liquidity
            )));
        }

        // Check Short Exchange (Bid side liquidity needed)
        let short_depth = depth_map.get(&opp.short_exchange).ok_or(
            RiskError::LowLiquidity(format!("No depth data for {}", opp.short_exchange))
        )?;

        let short_liquidity_sum: Decimal = short_depth.bids.iter().take(5)
             .map(|(p, s)| p * s)
             .sum();

        if short_liquidity_sum < required_liquidity {
            return Err(RiskError::LowLiquidity(format!(
                "{} Bids (Top 5): ${} < Required ${}", 
                opp.short_exchange, short_liquidity_sum, required_liquidity
            )));
        }

        Ok(())
    }

    /// Funding Rate Threshold: Block if predicted 24h loss > 50% of spread
    fn check_funding(
        &self,
        opp: &ArbitrageOpportunity,
        funding_map: &HashMap<ExchangeId, FundingInfo>,
    ) -> Result<(), RiskError> {
        // Funding usually paid every 8h. 
        // Long pays if Funding > 0. Short pays if Funding < 0.
        // Net Funding Cost = (Long_Rate + Short_Rate_reversed)
        // If we Long X and Short Y:
        // Cost Long = Rate_X (if positive, we pay. if negative, we receive)
        // Cost Short = -Rate_Y (if positive, we receive. if negative, we pay)
        
        let zero = Decimal::ZERO;
        let funding_long = funding_map.get(&opp.long_exchange).map(|f| f.rate_pct).unwrap_or(zero);
        let funding_short = funding_map.get(&opp.short_exchange).map(|f| f.rate_pct).unwrap_or(zero);

        // Predicted funding for 24h (3 payments)
        // Cost = (Rate_L - Rate_S) * 3 ? 
        // Example: 
        // Long X (Rate +0.01%): We pay 0.01%
        // Short Y (Rate +0.01%): We receive 0.01%
        // Net = 0.
        // Long X (Rate -0.01%): We receive 0.01%
        // Short Y (Rate -0.03%): We pay 0.03%
        // Net Payment = 0.02%

        let net_funding_per_interval = funding_long - funding_short;
        let net_funding_24h = net_funding_per_interval.abs() * Decimal::from(3);

        let threshold = opp.spread_pct.abs() * Decimal::new(5, 1); // 50% of spread

        if net_funding_24h > threshold {
             return Err(RiskError::HighFundingLoss(format!(
                 "Predicted 24h Funding Impact: {}% > Threshold {}%", 
                 net_funding_24h, threshold
             )));
        }

        Ok(())
    }

    /// Wallet Status Check
    fn check_wallet_status(
        &self,
        opp: &ArbitrageOpportunity,
        status_map: &HashMap<ExchangeId, AssetStatus>,
    ) -> Result<(), RiskError> {
        // Strict check: Both exchanges must be fully active
        if let Some(s) = status_map.get(&opp.long_exchange) {
            if !s.is_active || !s.can_withdraw { // Assume we might need to withdraw eventually
                return Err(RiskError::WalletDisabled(format!("{} wallet disabled/restricted", opp.long_exchange)));
            }
        }
        if let Some(s) = status_map.get(&opp.short_exchange) {
             if !s.is_active || !s.can_deposit {
                return Err(RiskError::WalletDisabled(format!("{} wallet disabled/restricted", opp.short_exchange)));
            }
        }
        Ok(())
    }

    /// Price Precision & Step Size Sync
    /// Rounds amount DOWN to the nearest Step Size
    pub fn normalize_amount(amount: Decimal, step_size: Decimal) -> Decimal {
        if step_size.is_zero() { return amount; }
        (amount / step_size).floor() * step_size
    }
}
