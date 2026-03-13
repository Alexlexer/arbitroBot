//! Application-wide constants to replace magic numbers.

use rust_decimal::Decimal;

/// Max age of ticker data (ms) before considered stale - reject arbitrage
pub const TICKER_STALE_MS: i64 = 15_000;

/// Target order volume (USDT) for arbitrage opportunities
pub fn target_volume_usdt() -> Decimal {
    Decimal::from(1000)
}

/// Approximate transfer/fee buffer (USDT) - require expected profit above this
pub fn transfer_fee_usdt() -> Decimal {
    Decimal::from(2)
}

/// Min expected profit (USDT) after spread - skip if below (expected_profit - transfer_fee > 0)
pub fn min_expected_profit_usdt() -> Decimal {
    transfer_fee_usdt()
}

/// Max slippage (as decimal, e.g. 0.001 = 0.1%) when computing safe volume from order book
pub fn max_slippage_for_safe_volume() -> Decimal {
    Decimal::new(1, 3) // 0.1%
}

/// Max lag (ms) for ticker timestamps in risk validation (must match TICKER_STALE_MS so opportunities aren't rejected after passing aggregator)
pub const RISK_MAX_TICKER_LAG_MS: i64 = TICKER_STALE_MS;

/// Min interval (ms) between Telegram alerts for the same symbol+pair - avoid spam
pub const OPPORTUNITY_ALERT_COOLDOWN_MS: i64 = 60_000;
