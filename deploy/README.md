# Deploy arbitroBot (dashboard at /arbitrobot)

## 1. One-shot deploy from your machine

From the repo root, with SSH access to the server:

```bash
chmod +x deploy/deploy.sh
./deploy/deploy.sh root@96.62.214.161
```

This will:

- Rsync the project to `/opt/arbitroBot` on the server (optionally copy `.env` if present)
- Run `docker compose build` and `docker compose up -d` on the server

## 2. Nginx: serve dashboard at `http://<server>/arbitrobot/`

On the server, add the arbitroBot location to your existing Nginx server block.

**Option A – paste into existing server block**

Edit your site config (e.g. `/etc/nginx/sites-available/default` or your vhost) and inside the `server { }` block add:

```nginx
location /arbitrobot/ {
    proxy_pass http://127.0.0.1:5174;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

**Option B – include file**

Copy the snippet to the server and include it:

```bash
sudo cp /opt/arbitroBot/deploy/nginx-arbitrobot.conf /etc/nginx/snippets/arbitrobot.conf
# In your server { } add:
#   include /etc/nginx/snippets/arbitrobot.conf;
```

Then:

```bash
sudo nginx -t
sudo systemctl reload nginx
```

## 3. RabbitMQ WebSocket (fix "Connecting to broker...")

The dashboard connects to RabbitMQ STOMP at `ws://<host>:15674/ws` (host = page hostname). Choose one:

**Option A – open port 15674 on the server** (simplest)

```bash
sudo ufw allow 15674/tcp
sudo ufw reload
```

Then open the dashboard as `http://<server-ip>:5174` (or via Nginx). The browser will use `ws://<server-ip>:15674/ws`.

**Option B – proxy WebSocket in Nginx** (no extra port)

Add to the same `server { }` block (before or after `/arbitrobot/`):

```nginx
location /arbitrobot-ws/ {
    proxy_pass http://127.0.0.1:15674/;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_read_timeout 86400;
}
```

Then rebuild the dashboard with the WebSocket URL pointing to this path:

```bash
cd dashboard
VITE_RABBITMQ_WS_URL=wss://YOUR_DOMAIN/arbitrobot-ws/ws npm run build
```

Use `wss://` if the site is HTTPS, `ws://` if HTTP. Deploy as usual; the built assets will use this URL.

**Option C – build-time URL** (direct IP/port)

If you always use the same server IP:

```bash
cd dashboard
VITE_RABBITMQ_WS_URL=ws://96.62.214.161:15674/ws npm run build
```

Then run the full deploy from the repo root (Docker will use the new build).

## 4. Checklist on server

- Docker and Docker Compose v2 installed
- `.env` present in `/opt/arbitroBot` (copy from your machine or create there)
- Port 5174 is bound by the dashboard container (only localhost; Nginx proxies to it).
- Port **15674** reachable from the browser (see §3), or dashboard built with `VITE_RABBITMQ_WS_URL`.

Dashboard URL: **http://\<server-ip\>:5174** or **http://\<server-ip\>/arbitrobot/** (if Nginx is configured).
