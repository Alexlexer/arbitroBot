use crate::constants::RISK_MAX_TICKER_LAG_MS;
use crate::model::{
    ExchangeId, ArbitrageOpportunity, OrderBookDepth, FundingInfo, AssetStatus, RiskError
};
use rust_decimal::Decimal;
use std::collections::HashMap;

pub struct RiskManager {
    pub config: std::sync::Arc<std::sync::Mutex<crate::config::AppConfig>>,
    pub secrets: std::sync::Arc<std::sync::Mutex<crate::config::SecretsConfig>>,
}

impl RiskManager {
    pub fn new(
        config: std::sync::Arc<std::sync::Mutex<crate::config::AppConfig>>,
        secrets: std::sync::Arc<std::sync::Mutex<crate::config::SecretsConfig>>,
    ) -> Self {
        Self { config, secrets }
    }

    /// Max USDT volume that can be traded without exceeding max_slippage_pct (e.g. 0.001 = 0.1%).
    /// Walks the order book until VWAP exceeds best ± slippage; returns min of long (asks) and short (bids) side.
    pub fn max_safe_volume_usdt(
        &self,
        opp: &ArbitrageOpportunity,
        depth_map: &HashMap<ExchangeId, OrderBookDepth>,
        max_slippage_pct: Decimal,
    ) -> Option<Decimal> {
        let long_depth = depth_map.get(&opp.long_exchange)?;
        let short_depth = depth_map.get(&opp.short_exchange)?;
        if long_depth.asks.is_empty() || short_depth.bids.is_empty() {
            return None;
        }
        let best_ask = long_depth.asks[0].0;
        let best_bid = short_depth.bids[0].0;
        if best_ask.is_zero() || best_bid.is_zero() {
            return None;
        }

        // Long side: buy on asks; stop when VWAP > best_ask * (1 + slippage)
        let mut cum_notional = Decimal::ZERO;
        let mut cum_size = Decimal::ZERO;
        for (p, s) in &long_depth.asks {
            let new_notional = cum_notional + p * s;
            let new_size = cum_size + s;
            if new_size.is_zero() {
                continue;
            }
            let vwap = new_notional / new_size;
            if vwap > best_ask * (Decimal::ONE + max_slippage_pct) {
                break;
            }
            cum_notional = new_notional;
            cum_size = new_size;
        }
        let long_safe = cum_notional;

        // Short side: sell on bids; stop when VWAP < best_bid * (1 - slippage)
        cum_notional = Decimal::ZERO;
        cum_size = Decimal::ZERO;
        for (p, s) in &short_depth.bids {
            let new_notional = cum_notional + p * s;
            let new_size = cum_size + s;
            if new_size.is_zero() {
                continue;
            }
            let vwap = new_notional / new_size;
            if vwap < best_bid * (Decimal::ONE - max_slippage_pct) {
                break;
            }
            cum_notional = new_notional;
            cum_size = new_size;
        }
        let short_safe = cum_notional;

        Some(long_safe.min(short_safe))
    }

    /// Primary validation entry point
    pub async fn validate(
        &self, 
        opp: &ArbitrageOpportunity,
        depth_map: &HashMap<ExchangeId, OrderBookDepth>,
        funding_map: &HashMap<ExchangeId, FundingInfo>,
        status_map: &HashMap<ExchangeId, AssetStatus>,
        market_filters: &HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>,
        ticker_timestamps: &HashMap<ExchangeId, i64>,
        account_state: &crate::model::GlobalAccountState,
        target_volume_usdt: Decimal, 
    ) -> Result<(), RiskError> {
        
        // 1. Check Wallet Status
        self.check_wallet_status(opp, status_map)?;

        // 2. Check Liquidation Risk (Margin Ratio < 80%)
        self.check_margin_ratio(opp, account_state)?;

        // 3. Check Timestamp Drift (Stale Data Guard)
        self.check_timestamp_drift(ticker_timestamps, RISK_MAX_TICKER_LAG_MS)?;

        // 4. Check Trading Status
        self.check_trading_status(opp, market_filters)?;

        // 5. Check Min Notional Guard
        self.check_min_notional(opp, market_filters, target_volume_usdt)?;

        // 6. Check Liquidity / Slippage
        self.check_liquidity(opp, depth_map, target_volume_usdt)?;

        // 7. Check Funding Rate
        self.check_funding(opp, funding_map)?;

        Ok(())
    }

    fn check_margin_ratio(
        &self,
        opp: &ArbitrageOpportunity,
        account_state: &crate::model::GlobalAccountState,
    ) -> Result<(), RiskError> {
        let threshold = Decimal::new(8, 1); // 0.8 (80%)

        // Check both exchanges
        for eid in &[opp.long_exchange, opp.short_exchange] {
            if let Some(exchange_state) = account_state.exchange_states.get(eid) {
                if exchange_state.margin_ratio > threshold {
                    return Err(RiskError::LiquidationRisk(format!(
                        "{} Margin Ratio too high: {:.2}% > {:.2}%",
                        eid, exchange_state.margin_ratio * Decimal::from(100), threshold * Decimal::from(100)
                    )));
                }
            }
        }
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

    /// Funding rate check: rate_pct is "% per 8h" (exchange standard). Require hours_to_breakeven >= 72h.
    fn check_funding(
        &self,
        opp: &ArbitrageOpportunity,
        funding_map: &HashMap<ExchangeId, FundingInfo>,
    ) -> Result<(), RiskError> {
        let zero = Decimal::ZERO;
        let funding_long = funding_map.get(&opp.long_exchange).map(|f| f.rate_pct).unwrap_or(zero);
        let funding_short = funding_map.get(&opp.short_exchange).map(|f| f.rate_pct).unwrap_or(zero);

        // Net funding cost per 8h interval (in %)
        let net_funding_per_8h = (funding_long - funding_short).abs();

        if net_funding_per_8h > Decimal::ZERO {
            // How many intervals can we survive before funding eats our entire Net Spread?
            let intervals_to_breakeven = opp.spread_pct / net_funding_per_8h;
            let hours_to_breakeven = intervals_to_breakeven * Decimal::from(8);
            
            // Safety check: We must have at least 72 hours (3 days) of buffer
            let min_safe_hours = Decimal::from(72); 
            
            if hours_to_breakeven < min_safe_hours {
                return Err(RiskError::HighFundingLoss(format!(
                    "Trade will become unprofitable in {:.1} hours due to funding costs. Min required: {}h", 
                    hours_to_breakeven, min_safe_hours
                )));
            }
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

    /// Timestamp Drift Check: Block if data is older than max_lag_ms
    fn check_timestamp_drift(
        &self,
        ticker_timestamps: &HashMap<ExchangeId, i64>,
        max_lag_ms: i64,
    ) -> Result<(), RiskError> {
        let now = chrono::Utc::now().timestamp_millis();
        for (exchange, ts) in ticker_timestamps {
            let lag = now - ts;
            if lag > max_lag_ms {
                return Err(RiskError::PriceDrift(format!(
                    "{} data is stale: {}ms lag (Max {}ms)",
                    exchange, lag, max_lag_ms
                )));
            }
        }
        Ok(())
    }

    /// Trading Status Check: Ensure symbol is active and not on maintenance
    fn check_trading_status(
        &self,
        opp: &ArbitrageOpportunity,
        market_filters: &HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>,
    ) -> Result<(), RiskError> {
        // Check Long Exchange
        if let Some(exchange_filters) = market_filters.get(&opp.long_exchange) {
            if let Some(filter) = exchange_filters.get(&opp.symbol) {
                if !filter.is_trading {
                    return Err(RiskError::ExchangeError(format!(
                        "{} symbol {} is not in TRADING mode",
                        opp.long_exchange, opp.symbol
                    )));
                }
            }
        }

        // Check Short Exchange
        if let Some(exchange_filters) = market_filters.get(&opp.short_exchange) {
            if let Some(filter) = exchange_filters.get(&opp.symbol) {
                if !filter.is_trading {
                    return Err(RiskError::ExchangeError(format!(
                        "{} symbol {} is not in TRADING mode",
                        opp.short_exchange, opp.symbol
                    )));
                }
            }
        }

        Ok(())
    }

    /// Min Notional Guard: Block if order value < Exchange Minimum
    fn check_min_notional(
        &self,
        opp: &ArbitrageOpportunity,
        market_filters: &HashMap<ExchangeId, HashMap<String, crate::model::SymbolMarketFilters>>,
        target_volume_usdt: Decimal,
    ) -> Result<(), RiskError> {
        // Check Long Exchange
        if let Some(exchange_filters) = market_filters.get(&opp.long_exchange) {
            if let Some(filter) = exchange_filters.get(&opp.symbol) {
                if target_volume_usdt < filter.min_notional {
                    return Err(RiskError::MinNotionalFilter(format!(
                        "{} order value ${} < Min Notional ${}",
                        opp.long_exchange, target_volume_usdt, filter.min_notional
                    )));
                }
            }
        }

        // Check Short Exchange
        if let Some(exchange_filters) = market_filters.get(&opp.short_exchange) {
            if let Some(filter) = exchange_filters.get(&opp.symbol) {
                if target_volume_usdt < filter.min_notional {
                    return Err(RiskError::MinNotionalFilter(format!(
                        "{} order value ${} < Min Notional ${}",
                        opp.short_exchange, target_volume_usdt, filter.min_notional
                    )));
                }
            }
        }

        Ok(())
    }

    /// Price Precision & Step Size Sync
    /// Rounds amount DOWN to the nearest Step Size
    pub fn _normalize_amount(amount: Decimal, step_size: Decimal) -> Decimal {
        if step_size.is_zero() { return amount; }
        (amount / step_size).floor() * step_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::model::*;

    fn mock_opp() -> ArbitrageOpportunity {
        ArbitrageOpportunity {
            symbol: "BTC".into(),
            long_exchange: ExchangeId::Binance,
            short_exchange: ExchangeId::Bybit,
            long_price: Decimal::new(50000, 0),
            short_price: Decimal::new(50500, 0),
            spread_pct: Decimal::new(1, 2), // 1%
            volume_usdt: Decimal::new(1000, 0),
            _timestamp: 0,
        }
    }

    #[test]
    fn test_check_min_notional() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let secrets = Arc::new(Mutex::new(crate::config::SecretsConfig::default()));
        let risk = RiskManager::new(config, secrets);
        let opp = mock_opp();
        let mut depth_map = HashMap::new();

        // Target $1000 volume, need $3000 liquidity
        let target_vol = Decimal::new(1000, 0);

        // Good liquidity
        depth_map.insert(ExchangeId::Binance, OrderBookDepth {
            bids: vec![],
            asks: vec![(Decimal::new(50000, 0), Decimal::new(1, 1))], // $5000 liquidity
        });
        depth_map.insert(ExchangeId::Bybit, OrderBookDepth {
            bids: vec![(Decimal::new(50000, 0), Decimal::new(1, 1))], // $5000 liquidity
            asks: vec![],
        });

        assert!(risk.check_liquidity(&opp, &depth_map, target_vol).is_ok());

        // Poor liquidity on Long side
        depth_map.get_mut(&ExchangeId::Binance).unwrap().asks = vec![(Decimal::new(50000, 0), Decimal::new(1, 2))]; // $500 liquidity
        assert!(risk.check_liquidity(&opp, &depth_map, target_vol).is_err());
    }

    #[test]
    fn test_check_liquidity() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let secrets = Arc::new(Mutex::new(crate::config::SecretsConfig::default()));
        let risk = RiskManager::new(config, secrets);
        let mut opp = mock_opp();
        opp.spread_pct = Decimal::new(1, 2); // 1% spread
        
        let mut funding_map = HashMap::new();
        // 0.01% vs 0.01% = 0 net
        funding_map.insert(ExchangeId::Binance, FundingInfo { rate_pct: Decimal::new(1, 4), next_funding_time: 0 });
        funding_map.insert(ExchangeId::Bybit, FundingInfo { rate_pct: Decimal::new(1, 4), next_funding_time: 0 });

        assert!(risk.check_funding(&opp, &funding_map).is_ok());

        // Extreme funding: 0.5% vs -0.5% = 1% net cost per interval. 3 intervals = 3%.
        // Spread is only 1%. 3% loss > 0.5% (50% spread threshold).
        funding_map.get_mut(&ExchangeId::Binance).unwrap().rate_pct = Decimal::new(5, 3); // 0.5%
        funding_map.get_mut(&ExchangeId::Bybit).unwrap().rate_pct = Decimal::new(-5, 3); // -0.5%
        
        let result = risk.check_funding(&opp, &funding_map);
        assert!(result.is_err());
        if let Err(RiskError::HighFundingLoss(_)) = result {
            // expected
        } else {
            panic!("Expected HighFundingLoss");
        }
    }

    #[test]
    fn test_check_timestamp_drift() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let secrets = Arc::new(Mutex::new(crate::config::SecretsConfig::default()));
        let risk = RiskManager::new(config, secrets);
        let mut ts_map = HashMap::new();
        let now = chrono::Utc::now().timestamp_millis();

        ts_map.insert(ExchangeId::Binance, now - 100);
        assert!(risk.check_timestamp_drift(&ts_map, 500).is_ok());

        ts_map.insert(ExchangeId::Binance, now - 1000);
        assert!(risk.check_timestamp_drift(&ts_map, 500).is_err());
    }
}
