# Security

## What is and isn’t encrypted

| What | Encrypted? | Details |
|------|------------|--------|
| **.env** | No | Stored as plain text on disk. `BOT_PASSWORD`, `TELEGRAM_BOT_TOKEN`, `INVITE_CODE`, and exchange API keys are readable by anyone with file access. |
| **config.json** | No | API keys are not stored in `config.json` (they are excluded from serialization). Dashboard-updated keys are appended to `.env` for supported exchanges. |
| **users.json** | No | Dashboard user accounts (username → Argon2 password hash). Keep file permissions restricted; do not commit to git. |
| **Process memory** | No | Passwords, tokens, and API keys are held as plain `String` in memory. |
| **Telegram API** | In transit | Requests to `api.telegram.org` use HTTPS (TLS). |
| **Exchange APIs** | In transit | All exchange requests use HTTPS (TLS) via `reqwest`. |
| **RabbitMQ** | No (default setup) | The stack uses `amqp://` (port 5672), not TLS. Traffic between bot, dashboard, and broker is unencrypted on the wire. |
| **Exchange signatures** | Cryptographic | API requests are signed with HMAC-SHA256. The secrets used for signing are not encrypted at rest; they are stored and loaded in plain text. |

**Summary:** Secrets are **not** encrypted at rest. Only transport is encrypted where HTTPS/TLS is used (Telegram and exchanges). RabbitMQ in the default compose is plain AMQP.

---

## Recommendations

1. **Keep secrets out of config.json**  
   Prefer loading API keys only from environment variables (e.g. `.env`). Avoid persisting `ApiCredentials` to `config.json`; if the dashboard updates keys, write them only to `.env` (or a dedicated secrets path) and do not serialize them into `AppConfig.save()`.

2. **Restrict .env and config**  
   Ensure `.env` is in `.gitignore`, give minimal file permissions (`chmod 600 .env`, and `chmod 600 config.json` if present), and keep these files only on the host that runs the bot. The script `run-docker.sh` applies `chmod 600` to `.env` and `config.json` before starting the stack.

3. **Use TLS for RabbitMQ in production**  
   For production, use `amqps://` and configure RabbitMQ with TLS so traffic between the bot, dashboard, and broker is encrypted.

4. **Secrets in Docker**  
   When running in Docker, use `env_file: .env` (as in the current compose) and avoid baking secrets into images. For stricter control, use Docker secrets or an external secrets manager instead of a plain `.env` file.

5. **API key scope**  
   Use exchange API keys with the minimum required permissions (e.g. read-only for monitoring, trading only if the bot is allowed to trade) and IP/withdrawal restrictions where the exchange supports them. See **API key scope** below for concrete steps.

6. **Future: secrets manager**  
   For multi-host or production deployments, consider a secrets manager (e.g. HashiCorp Vault, cloud provider secrets) and load credentials at runtime instead of from `.env` or config files.

---

## Production: RabbitMQ TLS

The default `docker-compose.yml` uses plain AMQP (`amqp://`). For production:

1. **Enable TLS in RabbitMQ**: Configure RabbitMQ with TLS (certificate and key). See [RabbitMQ TLS documentation](https://www.rabbitmq.com/ssl.html).
2. **Use amqps**: Set `RABBITMQ_URL=amqps://user:password@host:5671/` (port 5671 for AMQPS) in the bot and dashboard environment.
3. **Compose**: Either use a production compose that mounts certs and sets `RABBITMQ_USE_SSL=true` and related env vars, or run RabbitMQ behind a TLS-terminating proxy.

---

## API key scope

When creating exchange API keys for the bot:

- **Permissions**: Use the minimum needed (e.g. “Read” or “Read + Trade” only; avoid “Withdraw” or “Transfer” unless required).
- **IP allowlist**: If the exchange supports it, restrict the key to the IP(s) of the host(s) running the bot.
- **Withdrawals**: Disable withdrawal permission for bot keys.
- **Rotation**: Rotate keys periodically and after any suspected exposure.

---

## Reporting issues

If you find a security issue, please report it privately (e.g. to the repository owner) rather than in a public issue.
