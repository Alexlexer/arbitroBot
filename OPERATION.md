# Operation checklist

Use this to verify that all parts of the stack are working.

---

## 0. Deploy to server (dashboard at /arbitrobot)

To run on a shared server with the dashboard at **http://\<ip\>/arbitrobot/**:

1. **Deploy**: From repo root run `./deploy/deploy.sh root@<your-server>`. This syncs the project to `/opt/arbitroBot` and runs `docker compose up -d`.
2. **Nginx**: Add the `location /arbitrobot/` block from `deploy/nginx-arbitrobot.conf` into your existing Nginx server config, then `nginx -t` and `systemctl reload nginx`.
3. **Env**: Ensure `.env` exists in `/opt/arbitroBot` on the server (the script can copy it from your machine if you have it locally).

See `deploy/README.md` for full steps.

---

## 1. RabbitMQ

- **Ports**: 5672 (AMQP), 15672 (management UI), 15674 (STOMP/WebSocket).
- **Check**: Open http://localhost:15672 → login `guest` / `guest`. You should see the management UI.
- **STOMP**: The bot and dashboard use the same broker. The `rabbitmq_web_stomp` plugin must be enabled (done in `docker-compose`).

---

## 2. Bot (arbitro-bot)

- **Env**: Bot reads `.env` via `env_file` in Docker. Ensure `.env` exists next to `docker-compose.yml`.
- **Required for basic run**:
  - `RABBITMQ_URL` – set by compose to `amqp://guest:guest@rabbitmq:5672/`.
- **Optional** (recommended):
  - `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `BOT_PASSWORD` – for Telegram alerts and /login.
  - `INVITE_CODE` – for dashboard registration (if empty, registration is disabled).
- **Exchange balance / execution** (optional; for equity and execution):
  - Binance: `BINANCE_API_KEY`, `BINANCE_API_SECRET`
  - Bybit: `BYBIT_API_KEY`, `BYBIT_API_SECRET`
  - Bitget: `BITGET_API_KEY`, `BITGET_API_SECRET`, `BITGET_API_PASSPHRASE`
  - MEXC: `MEXC_API_KEY`, `MEXC_API_SECRET`
  - OKX: `OKX_API_KEY`, `OKX_API_SECRET`, `OKX_API_PASSPHRASE`
- **Check**: `docker compose logs arbitro-bot` – you should see “Aggregator started”, “Command consumer started”, and the arbitrage matrix. No “Failed to connect to RabbitMQ” or panic.

---

## 3. Dashboard

- **URL**: http://localhost:5173 (or http://\<your-host\>:5173 if accessing from another device).
- **WebSocket**: The dashboard connects to `ws://<same-host>:15674/ws`. If you open the dashboard from another machine, that machine must reach port 15674 on the host where Docker runs (e.g. port forwarding or same network).
- **Check**:
  - Green “System Live” = connected to RabbitMQ (STOMP).
  - Red “Connecting…” = browser cannot reach port 15674 (firewall, wrong host, or RabbitMQ not running).
- **Login**: Register (with `INVITE_CODE` from `.env`) or sign in with an existing user.

---

## 4. Telegram

- **Required**: `TELEGRAM_BOT_TOKEN`, `BOT_PASSWORD`. Optional: `TELEGRAM_CHAT_ID` (can be set via /login).
- **Check**: Send /start to the bot; then /login \<BOT_PASSWORD\>. If login works, alerts can be sent to that chat.
- **If “Login disabled”**: Set `BOT_PASSWORD` in `.env` and restart the bot.

---

## 5. Exchange API keys (balance + execution)

- **Balance**: If keys are set and valid, the dashboard “Balance monitor” and account summary show equity per exchange. If all show $0 or only one exchange, check keys and permissions (read/balance; no withdrawal unless needed).
- **Execution**:
  - **Default (simulated)**: If `ENABLE_LIVE_TRADING` is not set or not `true`, the bot only logs opportunities and does not send real orders.
  - **Live trading**: Set `ENABLE_LIVE_TRADING=true` in `.env` to send real **MARKET** orders. Implemented for **Binance USDT-M Futures** and **Bybit USDT perpetual**. Other exchanges are not implemented (orders are skipped with an error log). API keys must have **trade** permission.

---

## 6. Quick verification commands

```bash
# Containers up
docker compose ps

# Bot logs (last 50 lines)
docker compose logs arbitro-bot --tail 50

# Restart after .env changes
docker compose restart arbitro-bot
```

---

## 7. Common issues

| Symptom | What to check |
|--------|----------------|
| Dashboard stuck “Connecting…” | RabbitMQ running? Port 15674 open? Browser and Docker host same machine or correct host/port for WS? |
| Balance always $0 | Exchange API keys in `.env`? Keys have read/balance permission? Restart bot after changing `.env`. |
| “Registration disabled” / no invite | Set `INVITE_CODE` in `.env` and restart bot. |
| Telegram not responding | `TELEGRAM_BOT_TOKEN` and `BOT_PASSWORD` in `.env`; restart bot. |
| No trades | By default execution is simulated. Set `ENABLE_LIVE_TRADING=true` for real orders (Binance/Bybit only). Ensure risk checks pass and API keys have trade permission. |
