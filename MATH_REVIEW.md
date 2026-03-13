# Arbitrage Bot – Math Review

## ✅ What’s correct

### 1. Spread (aggregator)
- **Gross:** `(bid_short - ask_long) / ask_long * 100` — correct for long one exchange, short the other.
- **Costs:** Fees are doubled (open + close) per leg; slippage 0.2% total.  
  `total_cost_pct = (fee_long*2 + fee_short*2 + 0.2%) * 100`, `net_spread = gross - total_cost_pct`.

### 2. Funding (risk_manager)
- **Interpretation:** `rate_pct` is “% per 8h” (exchanges use 8h funding).  
  `intervals_to_breakeven = spread_pct / net_funding_per_8h`, `hours_to_breakeven = intervals * 8`.  
  Require `hours_to_breakeven >= 72` (3 days). Logic is correct.

### 3. Liquidity
- Require top-5 levels sum ≥ 3× target volume on both sides. Reasonable for slippage.

### 4. Min profit vs transfer
- We require `expected_profit_usdt > 2` (min_expected_profit_usdt). That is equivalent to “profit minus ~$2 transfer cost still positive” when the constant is treated as transfer buffer.

---

## ❌ Fixed in code

### 1. Stale-data / timestamp mismatch (bug)
- **Issue:** Aggregator allows data up to **15 s** old (`TICKER_STALE_MS`), but risk used **500 ms** (`RISK_MAX_TICKER_LAG_MS`). So every opportunity that passed the aggregator was then rejected by risk as “stale.”
- **Fix:** Risk now uses the same threshold as the aggregator (single constant `TICKER_STALE_MS`).

---

## 📋 Documented / optional improvements

### 1. Transfer fee in the formula — **done**
- **Implemented:** `transfer_fee_usdt()` constant (2 USDT); we require `expected_profit_usdt > transfer_fee_usdt()` so profit after transfer/fee buffer is positive.

### 2. Dynamic position size from order book — **done**
- **Implemented:** `RiskManager::max_safe_volume_usdt(opp, depth_map, max_slippage_pct)` walks long asks and short bids until VWAP exceeds best ± slippage (0.1%); returns min of the two sides. Aggregator uses `target_volume = min(safe_volume, 1000)` for expected profit and validation.

### 3. Funding rate period
- **Clarity:** Comment in code (or in this doc) that funding is **per 8h** so `rate_pct` and `hours_to_breakeven` are interpreted correctly. Done in comments.

### 4. Price ratio sanity
- **Bot (detect_sharps):** Reject if `price_ratio > 11` to avoid unit/symbol mismatch.
- **Dashboard:** Reject if price ratio > 1.5 (50%) and cap displayed spread at 50%. That filters obvious symbol collision (e.g. 85% where "M" is different on each exchange) while allowing real large spreads (e.g. 30%).

---

## Summary

- Spread, fees, slippage, funding breakeven, and min-profit logic are correct.
- The only real bug found was the **stale-data threshold mismatch** (15 s vs 500 ms); that is fixed by using one shared constant for both aggregator and risk.
