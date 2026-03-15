#!/usr/bin/env bash
# Build and run ArbitroBot stack (RabbitMQ + bot + dashboard)
set -e
cd "$(dirname "$0")"

if ! command -v docker &>/dev/null; then
  echo "Docker is not installed or not in PATH. Install Docker Desktop and try again."
  exit 1
fi

# Create .env from example if missing
if [ ! -f .env ]; then
  echo "No .env found. Copying .env.example to .env - edit .env and set TELEGRAM_BOT_TOKEN, BOT_PASSWORD, etc."
  cp -n .env.example .env 2>/dev/null || true
fi

# Restrict permissions on secrets/config (see SECURITY.md)
chmod 600 .env 2>/dev/null || true
[ -f config.json ] && chmod 600 config.json 2>/dev/null || true
[ -f users.json ] && chmod 600 users.json 2>/dev/null || true

echo "Building and starting containers..."
docker compose up --build -d

echo ""
echo "Services:"
echo "  - RabbitMQ:     localhost:5672 (AMQP), localhost:15672 (management UI)"
echo "  - Dashboard:    http://localhost:5174"
echo "  - Bot:          running in background (no exposed port)"
echo ""
echo "Logs:  docker compose logs -f arbitro-bot"
echo "Stop:  docker compose down"
