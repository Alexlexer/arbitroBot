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
    proxy_pass http://127.0.0.1:5173;
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

## 3. Checklist on server

- Docker and Docker Compose v2 installed
- `.env` present in `/opt/arbitroBot` (copy from your machine or create there)
- Port 5173 is bound by the dashboard container (only localhost; Nginx proxies to it).
- The dashboard connects to RabbitMQ STOMP at `ws://<server-ip>:15674/ws`. Either expose port 15674 to the internet (so the browser can connect), or add an Nginx WebSocket proxy for `/arbitrobot-ws/` → `127.0.0.1:15674` and change the dashboard to use that path.

Dashboard URL: **http://\<server-ip\>/arbitrobot/**
