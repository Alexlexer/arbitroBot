# ArbitroBot Development Plan (P0–P3)

## P0 – Critical (Done ✅)

| # | Item | Fix |
|---|------|-----|
| 1 | Missing `Mutex` import in `main.rs` | Added `use std::sync::{Arc, Mutex}` |
| 2 | Config access bug in `aggregator.rs` line 286 | Replaced `self.config.min_spread_threshold` with locked access |
| 3 | Default password `"admin123"` in `notifier.rs` | Removed fallback; require `BOT_PASSWORD` in `.env`; login disabled if empty |

## P1 – High (Done ✅)

| # | Item | Fix |
|---|------|-----|
| 4 | Panic-prone `unwrap()` on Mutex locks | Replaced with `unwrap_or_else(\|e\| e.into_inner())` in aggregator |
| 5 | Rebalance spam (every 2s) | Debounce: only send Telegram when advice changes (`last_rebalance_notified`) |

## P2 – Medium (Done ✅)

| # | Item | Fix |
|---|------|-----|
| 6 | MEXC symbol normalization inconsistent | Switched to `normalize_symbol()` for consistency with other exchanges |
| 7 | Transfer cost ignored in rebalance | Added `MIN_TRANSFER_AFTER_FEE_USDT` (25 USDT); skip transfers below this |

## P3 – Lower (Done ✅)

| # | Item | Fix |
|---|------|-----|
| 8 | Magic numbers | Added `src/constants.rs`: `TICKER_STALE_MS`, `TARGET_VOLUME_USDT`, `MIN_EXPECTED_PROFIT_USDT`, `RISK_MAX_TICKER_LAG_MS` |
| 9 | WebSocket reconnection | Exponential backoff: `reconnect_delay_secs(attempt)` → 2, 4, 8, 16, 32, 64s (capped at 120s) in Binance, Bybit, Bitget |

## Files Changed

- `src/main.rs` – Mutex import, logger expect, poison-safe lock
- `src/aggregator.rs` – Config fix, poison-safe locks, debounce, constants
- `src/notifier.rs` – No default password, login guard
- `src/model.rs` – `PartialEq` for `RebalanceAdvice`
- `src/rebalance_advisor.rs` – Transfer cost constants
- `src/risk_manager.rs` – Use `RISK_MAX_TICKER_LAG_MS`
- `src/constants.rs` – New file with shared constants
- `src/exchange/mod.rs` – `reconnect_delay_secs()` helper
- `src/exchange/binance.rs` – Backoff, URL expect
- `src/exchange/bybit.rs` – Backoff, URL expect
- `src/exchange/bitget.rs` – Backoff, URL expect
- `src/exchange/mexc.rs` – `normalize_symbol()`
