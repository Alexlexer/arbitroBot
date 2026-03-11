# Arbitrage Bot Formula Improvements

This document outlines critical flaws in the current mathematical formulas used for arbitrage execution and provides concrete suggestions for replacing them with safer, more realistic calculations.

## 1. Spread Calculation (The "Hidden Exit Fee" Problem)

**File:** `src/aggregator.rs`

### Current Flaw
The current `gross_spread` to `net_spread` calculation only accounts for the cost of **opening** the positions (1x Taker Fee + Slippage). In perpetual futures arbitrage, you must eventually **close** the positions when prices converge to realize the profit. This means paying fees and slippage a second time.

```rust
// Current (Flawed)
let total_cost_pct = (fee_long + fee_short + slippage) * Decimal::from(100);
```

### Proposed Improvement
Double the fees and slippage assumptions to account for a full round-trip trade (Open + Close).

```rust
// Proposed (Safer)
let fee_long_total = long.exchange.taker_fee() * Decimal::from(2); // Open + Close
let fee_short_total = short.exchange.taker_fee() * Decimal::from(2); // Open + Close
let slippage_total = Decimal::new(2, 3); // 0.2% total buffer (0.1% entry + 0.1% exit)

let total_cost_pct = (fee_long_total + fee_short_total + slippage_total) * Decimal::from(100);
let net_spread = gross_spread - total_cost_pct;
```

---

## 2. Funding Rate Risk (The "Time to Convergence" Flaw)

**File:** `src/risk_manager.rs`

### Current Flaw
The `check_funding` function hardcodes an assumption that you will hold the trade for exactly 24 hours (3 funding intervals). It compares this projected cost against 50% of your spread profit.
1. Some divergent spreads can take a week to close. 
2. Funding rates change every 8 hours.

```rust
// Current (Flawed)
let net_funding_24h = net_funding_per_interval.abs() * Decimal::from(3);
let threshold = opp.spread_pct.abs() * Decimal::new(5, 1); // 50% of spread 
```

### Proposed Improvement
Implement a "Max Hold Time" or "Breakeven Time" calculation. Instead of deciding if 24h is acceptable, the bot should calculate exactly how many hours it can hold the trade before it starts losing money, and ensure that number is reasonably high.

```rust
// Proposed (Safer)
let net_funding_per_8h = (funding_long - funding_short).abs();

// Prevent dividing by zero if funding is exactly neutral
if net_funding_per_8h > Decimal::ZERO {
    // How many intervals can we survive before funding eats our entire Net Spread?
    let intervals_to_breakeven = opp.spread_pct / net_funding_per_8h;
    let hours_to_breakeven = intervals_to_breakeven * Decimal::from(8);
    
    // Safety check: We must have at least 72 hours (3 days) of buffer
    let min_safe_hours = Decimal::from(72); 
    
    if hours_to_breakeven < min_safe_hours {
        return Err(RiskError::HighFundingLoss(format!(
            "Trade will become unprofitable in {} hours due to funding costs. Min required: {}h", 
            hours_to_breakeven.round_dp(1), min_safe_hours
        )));
    }
}
```

---

## 3. Position Sizing (Adding "Max Sizing by Liquidity")

**File:** `src/risk_manager.rs`

### Current Flaw
Currently, `target_volume` is a hardcoded `1000 USDT`. `check_liquidity` just ensures the top 5 order book levels have $3,000 in volume. While this prevents slippage on a $1,000 order, it doesn't dynamically scale.

### Proposed Improvement
Instead of a boolean pass/fail on a static $1000 target, the Risk Manager should calculate the **Maximum Safe Volume** based on the order book.

```rust
// Proposed Logic (Conceptual)
// 1. Scan the asks on the Long exchange until cumulative volume reaches a target slippage limit (e.g. 0.1%).
// 2. Do the same for bids on the Short exchange.
// 3. Find the lowest volume of the two.
// 4. Return this dynamic `safe_volume` to the Aggregator.
// 5. The Aggregator executes exactly `safe_volume` instead of a static 1000 USDT.
```

---

## 4. Rebalancing Transfer Costs

The bot has a `rebalance_advisor.rs` to move funds when one exchange runs out of USDT. Currently, the spread calculation **does not account for the network fee** of transferring USDT over TRC20/ERC20 (approx $1 to $5). 

**Proposed Action:**
Subtract a static $2 from the raw expected profit ($ amount, not percentage) before verifying the trade is worth it. For small volume trades (e.g. $100), a $2 transfer fee might completely negate a 1.5% profit.
