#!/usr/bin/env bash
# Deploy arbitroBot to a remote server at /opt/arbitroBot and serve dashboard at /arbitrobot
# Usage: ./deploy/deploy.sh [user@host]
# Example: ./deploy/deploy.sh root@96.62.214.161
# Requires: ssh access, rsync. On server: Docker and Docker Compose v2.

set -e
REMOTE="${1:-root@96.62.214.161}"
APP_DIR="/opt/arbitroBot"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "Deploying from $REPO_ROOT to $REMOTE:$APP_DIR"

# Sync project (exclude .git, node_modules, target to keep transfer small)
rsync -avz --delete \
  --exclude '.git' \
  --exclude 'dashboard/node_modules' \
  --exclude 'target' \
  --exclude '.env' \
  "$REPO_ROOT/" "$REMOTE:$APP_DIR/"

# Copy .env if it exists locally (optional; server may already have it)
if [ -f "$REPO_ROOT/.env" ]; then
  echo "Copying .env to server..."
  scp "$REPO_ROOT/.env" "$REMOTE:$APP_DIR/.env"
fi

echo "Running docker compose on server..."
ssh "$REMOTE" "cd $APP_DIR && docker compose build && docker compose up -d"

echo "Done. Ensure nginx is configured for /arbitrobot/ (see deploy/nginx-arbitrobot.conf)."
echo "Dashboard URL: http://<server-ip>/arbitrobot/"
