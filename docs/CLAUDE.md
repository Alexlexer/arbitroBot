# CLAUDE.md — Arbitrage Hub

## Project Overview

High-performance cryptocurrency arbitrage trading bot written in Rust. Monitors price spreads across 8 exchanges in real-time via WebSocket feeds, detects profitable opportunities, validates through a multi-layer risk pipeline, and optionally executes hedged trades. Includes a Telegram bot, RabbitMQ-backed dashboard, SQLite history store, and a Vue.js frontend.

**Default mode: Simulated** (no real trades unless `ENABLE_LIVE_TRADING=true`).

---

## Repository Structure

```
arbit_hub/
├── src/
│   ├── main.rs              # Entry point — spawns all async tasks
│   ├── aggregator.rs        # Core: market matrix, opportunity detection, broadcast
│   ├── execution.rs         # Order placement (Binance + Bybit live; others simulated)
│   ├── risk_manager.rs      # 7-stage validation pipeline before execution
│   ├── poller.rs            # REST polling: funding rates, market filters, balances
│   ├── notifier.rs          # Telegram bot (/login, /status, /ping, alerts)
│   ├── messaging.rs         # RabbitMQ AMQP client (publishes state/config/alerts)
│   ├── history.rs           # SQLite store + Axum REST API (GET /api/history)
│   ├── listener.rs          # Optional external WebSocket alert listener
│   ├── rate_limiter.rs      # Token bucket per exchange (REST + order limits)
│   ├── rebalance_advisor.rs # Fund movement recommendations across exchanges
│   ├── transfers.rs         # Withdrawal execution
│   ├── auth.rs              # Login/password handling
│   ├── config.rs            # AppConfig + SecretsConfig loading (JSON + .env)
│   ├── model.rs             # All core data structures and enums
│   ├── logger.rs            # In-memory log buffer (last 20 lines for dashboard)
│   ├── constants.rs         # Magic numbers (stale threshold, target volume, etc.)
│   └── exchange/            # WebSocket clients — one file per exchange
│       ├── mod.rs
│       ├── binance.rs       # wss://fstream.binance.com — depth5@100ms
│       ├── bybit.rs         # wss://stream.bybit.com/v5/public/linear — depth20
│       ├── bitget.rs        # wss://ws.bitget.com/mix/v1/public — depth20
│       ├── mexc.rs
│       ├── bitmart.rs
│       ├── kraken.rs
│       └── gate.rs
├── dashboard/               # Vue.js / Vite frontend
├── deploy/                  # deploy.sh, nginx config
├── Cargo.toml
├── Dockerfile               # Multi-stage Rust build
├── docker-compose.yml       # RabbitMQ + bot + dashboard
├── .env                     # Secrets (never commit)
├── config.json              # Runtime config (persisted by bot)
├── OPERATION.md             # Deployment checklist
├── SECURITY.md              # Security recommendations
└── MATH_REVIEW.md           # Formula validation
```

---

## Architecture

Actor-based async architecture using Tokio:

```
WebSocket Feeds (7 exchanges)
        │ UnifiedTicker channels (mpsc)
        ▼
  AGGREGATOR (main loop)
  - Builds market matrix: symbol → exchange → best bid/ask
  - Detects spreads across all exchange pairs
  - Validates via RiskManager
  - Sends to ExecutionActor if valid
  - Broadcasts state via RabbitMQ every 2s
  - Snapshots top 30 opps to SQLite every 5min
        │
   ┌────┴───────┬──────────────┐
   ▼            ▼              ▼
EXECUTION   TELEGRAM      RABBITMQ
ACTOR       NOTIFIER      (dashboard sync)
```

**Key async tasks spawned in `main.rs`:**
1. Exchange WebSocket launchers (one per exchange)
2. Data poller (funding rates, balances — REST every 30s / 1h)
3. Telegram notifier
4. Aggregator
5. Axum HTTP server (port 8080)
6. RabbitMQ command consumer

---

## Supported Exchanges

| Exchange | Feed | Live Execution | Balance | Funding |
|----------|------|---------------|---------|---------|
| Binance  | WS depth5@100ms | Yes (USDT-M Futures) | Yes | Yes |
| Bybit    | WS depth20@100ms | Yes (Perps) | Yes | Yes |
| Bitget   | WS depth20@100ms | No | Yes | Yes |
| MEXC     | WS depth20@100ms | No | Yes | No |
| Bitmart  | WS depth50 | No | Limited | No |
| Kraken   | WS book | No | Limited | No |
| Gate.io  | WS depth20 | No | Limited | No |
| OKX      | REST only | No | Yes | Yes |

All markets are **USDT-margined perpetual futures**.

---

## Core Data Flow

### Opportunity Detection (`aggregator.rs`)

```
Gross Spread % = (best_bid_short - best_ask_long) / best_ask_long * 100
Net Spread %   = Gross - (fee_long*2 + fee_short*2 + slippage_0.2%)
Profit ($)     = volume_usdt * net_spread_pct / 100
Valid if       = Profit > $2 (transfer fee buffer)
```

Taker fees (hardcoded in `model.rs`, can be overridden in `config.json`):
- Binance: 0.05%, Bybit: 0.06%, Bitget: 0.06%, MEXC: 0.06%
- Bitmart: 0.20%, Kraken: 0.10%, Gate.io: 0.075%, OKX: 0.05%

### Risk Validation Pipeline (`risk_manager.rs`)

In order — any failure aborts:
1. **Wallet Status** — USDT deposit/withdrawal enabled on both exchanges
2. **Margin Ratio** — both exchanges < 80% (liquidation protection)
3. **Stale Data** — ticker timestamp < 5 seconds old
4. **Trading Status** — symbol is "TRADING" on both exchanges
5. **Min Notional** — volume meets exchange minimums
6. **Liquidity** — top 5 book levels have ≥ 3× target volume; VWAP within ±0.1% slippage
7. **Funding Rate** — net funding breakeven ≥ 72 hours

### Execution (`execution.rs`)

**Live mode** (`ENABLE_LIVE_TRADING=true`):
- Places parallel MARKET orders on both exchanges
- Rollback: if one leg fails after the other succeeds, reverses immediately
- Only Binance USDT-M Futures and Bybit Perpetual are implemented; others log "not implemented"

**Simulated mode** (default):
- Logs opportunity, simulates fill, records in trade history (no real orders)

---

## Configuration

### `.env` (Secrets — never commit)

```bash
BINANCE_API_KEY=
BINANCE_API_SECRET=
BYBIT_API_KEY=
BYBIT_API_SECRET=
BITGET_API_KEY=
BITGET_API_SECRET=
BITGET_API_PASSPHRASE=
MEXC_API_KEY=
MEXC_API_SECRET=
OKX_API_KEY=
OKX_API_SECRET=
OKX_API_PASSPHRASE=

TELEGRAM_BOT_TOKEN=
TELEGRAM_CHAT_ID=
BOT_PASSWORD=

INVITE_CODE=
ENABLE_LIVE_TRADING=     # Set "true" or "1" to place real orders
HISTORY_DB_PATH=data/arbitro_history.db
HISTORY_API_PORT=8080
RABBITMQ_URL=amqp://guest:guest@localhost:5672/
```

### `config.json` (Runtime — persisted by bot, can be changed via dashboard)

```json
{
  "min_spread_threshold": 5,       // Min spread % to act on
  "depth_usdt": 1000,              // Order size in USDT
  "listener_ws_url": null,         // External alert listener URL
  "enabled_exchanges": {...},      // Toggle per exchange
  "api_keys": {},                  // Runtime key overrides (prefer .env)
  "taker_fee_overrides": {},       // Override per-exchange fees
  "margin_threshold_low": 0.4,     // 40% — rebalance advisory
  "margin_threshold_high": 0.7,    // 70% — urgent rebalance
  "concentration_threshold": 0.7,  // 70% — single-exchange risk
  "target_margin_ratio": 0.2,      // 20% — safe margin level
  "polling_interval_ms": 5000
}
```

---

## Key Constants (`src/constants.rs`)

| Constant | Value | Meaning |
|----------|-------|---------|
| TICKER_STALE_MS | 5,000 ms | Reject data older than 5s |
| TARGET_VOLUME_USDT | $1,000 | Default order size |
| MIN_PROFIT_USD | $2 | Transfer fee buffer |
| MAX_SLIPPAGE | 0.1% | VWAP tolerance on book |
| MIN_LIQUIDITY_FACTOR | 3× | Top 5 levels vs target |
| FUNDING_BREAKEVEN_HRS | 72h | Min hours to funding break-even |

---

## External Interfaces

### RabbitMQ (AMQP)
- **Broker:** `localhost:5672` (Docker service)
- **Exchange:** `arbit_hub` (topic, durable)
- **Publish routing keys:** `state` (every 2s), `config`, `listener.alert`
- **Consume queue:** `bot_commands` (dashboard → bot)
- **STOMP WebSocket:** port 15674 (dashboard subscribes here)

### REST API (Axum, port 8080)
- `GET /health` → `"ok"`
- `GET /api/history?days=30` → JSON snapshots from SQLite

### Telegram Bot
- `/start` — show menu and chat ID
- `/login <BOT_PASSWORD>` — authenticate for alerts
- `/status` — balances, positions, system state
- `/ping` — connectivity check
- Alerts sent for spreads > 5% (adaptive cooldown: 10s at 3%+, 30s at 1%+, 60s default)

### Listener (optional)
- External WebSocket alerts (new token discoveries, etc.)
- JSON: `{ "type": "alert", "symbol": "...", "exchange": "...", ... }`

---

## .NET Backend (`backend/`)

ASP.NET Core 8 backend that sits between the Rust bot and the React dashboard.
Replaces the old direct RabbitMQ STOMP connection from the browser with SignalR.

### Architecture

```
React Dashboard  ←→  SignalR /hubs/arbit  ←→  RabbitMqBridgeService  ←→  RabbitMQ  ←→  Rust Bot
                      REST /api/auth             (IHostedService)
                      REST /api/history  ──proxy──►  Rust Axum :8080
```

### Key files

| File | Purpose |
|------|---------|
| `Program.cs` | DI, JWT, CORS, SignalR, Swagger wiring |
| `Hubs/ArbitSignalRHub.cs` | Typed SignalR hub — `[Authorize]`, `SendBotCommand` |
| `Services/RabbitMqBridgeService.cs` | Background service — subscribes to `arbit_hub` exchange, fans out to SignalR clients |
| `Services/AuthService.cs` | BCrypt + SQLite user store + JWT issuance |
| `Services/HistoryProxyService.cs` | HTTP proxy to Rust Axum history API |
| `Controllers/AuthController.cs` | `POST /api/auth/login`, `POST /api/auth/register` |
| `Controllers/HistoryController.cs` | `GET /api/history?days=N` (JWT required) |
| `appsettings.json` | All config — **change Jwt:Key and Auth:InviteCode before deploy** |

### SignalR hub methods

**Server → Client** (`IArbitHubClient` interface):
- `ReceiveState(JsonElement)` — full bot state every ~2s
- `ReceiveBotConfig(JsonElement)` — config changes
- `ReceiveListenerAlert(JsonElement)` — external listener alert
- `ReceiveListenerOpportunity(JsonElement)` — best opp for alerted symbol
- `ReceiveConnectionStatus(bool)` — RabbitMQ connectivity

**Client → Server**:
- `SendBotCommand(JsonElement)` — forwarded to RabbitMQ `bot.commands`

### Auth flow

1. `POST /api/auth/login` → `{ token, username }`
2. SignalR: connect with `?access_token=<token>` in the URL
3. REST: `Authorization: Bearer <token>`

### Critical config (must change before deploy)

```json
"Jwt":  { "Key": "≥32 char random secret" },
"Auth": { "InviteCode": "your-code", "UsersDbPath": "data/users.db" }
```

Env var override uses `__` separator: `Jwt__Key`, `Auth__InviteCode`, `RabbitMq__Host`.

Docker Compose service: `arbitro-backend` on host port `5000`.

---

## Build & Run

### Local Development

```bash
# Copy and fill in secrets
cp .env.example .env

# Run with Docker Compose (RabbitMQ + bot + .NET backend + dashboard)
./run-docker.sh

# Rust bot (native)
cargo build --release
cargo run --release

# .NET backend (native)
cd backend
dotnet run
# Swagger UI at http://localhost:5000/swagger
```

### Production Deployment

```bash
./deploy/deploy.sh
# Deploys to /opt/arbitroBot on remote server
# Nginx reverse proxy config: deploy/nginx-arbitrobot.conf
# Dashboard accessible at /arbitrobot/
```

### Docker Services

| Service | Ports |
|---------|-------|
| `rabbitmq` | 5672 (AMQP), 15672 (management UI), 15674 (STOMP/WS) |
| `arbitro-bot` | 9180→8080 (history API) |
| `arbitro-dashboard` | 5174 |

---

## Development Guidelines

### Adding a New Exchange

1. Create `src/exchange/<name>.rs` implementing the `Exchange` trait (`connect()` + `subscribe()`)
2. Add variant to `ExchangeId` enum in `src/model.rs` with its taker fee
3. Register the launcher in `src/main.rs`
4. Add polling endpoints in `src/poller.rs` if funding/balance data available
5. Add execution implementation in `src/execution.rs` if live trading needed
6. Add rate limits in `src/rate_limiter.rs`

### Adding Risk Checks

All checks live in `src/risk_manager.rs`. The pipeline runs sequentially — add new checks by inserting a new validation step. Return `Err(String)` to reject; `Ok(())` to pass.

### Modifying Spread Formula

Core formula is in `src/aggregator.rs`. The `constants.rs` file holds thresholds. The `MATH_REVIEW.md` documents the expected behavior — update it when changing the formula.

### Trade History

In-memory buffer: 1,000 entries max (oldest 500 trimmed when full). SQLite snapshots top 30 opportunities every 5 minutes. DB auto-cleans if it exceeds 10 GB.

---

## Important Caveats

- **Only Binance + Bybit have live execution.** Other exchanges detect opportunities but cannot execute — they are monitoring-only.
- **Rollback is best-effort.** If the reversal order also fails (e.g., network loss), the position is left open. Manual intervention may be required.
- **Funding rate check requires position data.** If balance polling has not completed yet at startup, the funding check may pass vacuously — allow 30–60 seconds after startup before trusting execution.
- **API keys scoping:** Use trade-only keys (no withdrawal permission) unless the rebalance/transfer feature is explicitly enabled.
- **Secrets in plain text at rest.** `.env` and `config.json` must be `chmod 600`. See `SECURITY.md` for full recommendations.

---

## Dependencies Summary

| Category | Crates |
|----------|--------|
| Async runtime | `tokio` (full features) |
| WebSocket | `tokio-tungstenite`, `futures-util` |
| HTTP client | `reqwest` (0.11, JSON + TLS) |
| Serialization | `serde`, `serde_json` |
| Crypto signing | `hmac`, `sha2`, `hex`, `base64` |
| Decimal math | `rust_decimal` (1.32) |
| Database | `rusqlite` (0.31) |
| Web framework | `axum` (0.7) + `tower-http` |
| Messaging | `amqprs` (2.1.3) |
| Auth | `argon2` |
| Utilities | `chrono`, `log`, `env_logger`, `dotenvy`, `crossterm`, `prettytable-rs` |
