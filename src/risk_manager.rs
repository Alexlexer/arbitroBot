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
        self.check_timestamp_drift(ticker_timestamps, 500)?;

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
            _timestamp: 0,
        }
    }

    #[test]
    fn test_check_margin_ratio() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let risk = RiskManager::new(config);
        let opp = mock_opp();
        let mut state = GlobalAccountState {
            total_equity_usdt: Decimal::new(2000, 0),
            total_unrealized_pnl: Decimal::ZERO,
            exchange_states: HashMap::new(),
            asset_statuses: HashMap::new(),
            config: crate::config::AppConfig::default(),
        };

        // Safe state
        state.exchange_states.insert(ExchangeId::Binance, ExchangeAccountState {
            total_equity: Decimal::new(1000, 0),
            available_balance: Decimal::new(500, 0),
            margin_ratio: Decimal::new(5, 1), // 0.5 (50%)
            positions: vec![],
        });
        
        assert!(risk.check_margin_ratio(&opp, &state).is_ok());

        // Unsafe state (90% margin)
        state.exchange_states.get_mut(&ExchangeId::Binance).unwrap().margin_ratio = Decimal::new(9, 1);
        let result = risk.check_margin_ratio(&opp, &state);
        assert!(result.is_err());
        if let Err(RiskError::LiquidationRisk(msg)) = result {
            assert!(msg.contains("Margin Ratio too high"));
        } else {
            panic!("Expected LiquidationRisk");
        }
    }

    #[test]
    fn test_check_liquidity() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let risk = RiskManager::new(config);
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
    fn test_check_funding() {
        let config = Arc::new(Mutex::new(crate::config::AppConfig::default()));
        let risk = RiskManager::new(config);
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
        let risk = RiskManager::new(config);
        let mut ts_map = HashMap::new();
        let now = chrono::Utc::now().timestamp_millis();

        ts_map.insert(ExchangeId::Binance, now - 100);
        assert!(risk.check_timestamp_drift(&ts_map, 500).is_ok());

        ts_map.insert(ExchangeId::Binance, now - 1000);
        assert!(risk.check_timestamp_drift(&ts_map, 500).is_err());
    }
}
