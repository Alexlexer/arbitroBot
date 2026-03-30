# Listener — интеграция в свой проект

Документ описывает, как работает **Listener** (сервис рассылки алертов о новых листингах) и как подключить к нему свой бот или приложение.

---

## 1. Что такое Listener

Listener — это центральный сервис, который:

1. **Получает** события о новых листингах с бирж (через внешний источник: NewListings.pro, ListingFeed или свой фид).
2. **Фильтрует** их по настройкам (биржи, spot/futures, pre-market, launchpool и т.д.).
3. **Рассылает** отфильтрованные алерты всем подключённым клиентам по **WebSocket**.
4. Предоставляет **HTTP API** для управления конфигом, списком трейдеров и ручной отправки тестового алерта.

Ваш проект подключается к Listener как **клиент WebSocket**: один раз установили соединение — получаете все алерты в реальном времени.

---

## 2. Порты и адреса

| Сервис        | Порт по умолчанию | Назначение                          |
|---------------|-------------------|-------------------------------------|
| **HTTP API**  | `8082`            | Конфиг, статус, тест, traders       |
| **WebSocket** | `8083`            | Рассылка алертов и config_update    |

Переменные окружения (на стороне Listener):

- `PORT` — порт HTTP API (по умолчанию 8082).
- `WS_PORT` — порт WebSocket (по умолчанию 8083).
- `LISTENER_AUTH_TOKEN` — токен для HTTP API (заголовок `X-Auth-Token`).
- `BOT_KEY` — опционально; если задан, клиенты WebSocket должны подключаться с `?token=<BOT_KEY>`.

Пример базовых URL:

- API: `http://listener-host:8082`
- WebSocket: `ws://listener-host:8083/ws`

Если включён `BOT_KEY`:

- WebSocket: `ws://listener-host:8083/ws?token=YOUR_BOT_KEY`

---

## 3. Подключение по WebSocket

### 3.1 URL

```
ws://<host>:<WS_PORT>/ws
```

Если у Listener задан `BOT_KEY`:

```
ws://<host>:<WS_PORT>/ws?token=<BOT_KEY>
```

Без правильного `token` при включённом `BOT_KEY` соединение закрывается с 401.

### 3.2 Формат сообщений (JSON)

Все сообщения — JSON с полем `type`. Клиент должен слать **pong** на **ping** (опционально, но рекомендуется для keepalive).

#### Входящие от Listener

**Алерт (новый листинг):**

```json
{
  "type": "alert",
  "symbol": "BTC_USDT",
  "exchange": "MEXC",
  "kind": "futures",
  "source": "listing-detector",
  "reason": "new_listing",
  "at": 1710000000000,
  "mcap_usd": 50000000.5
}
```

| Поле       | Тип    | Описание |
|------------|--------|----------|
| `type`     | string | `"alert"` |
| `symbol`   | string | Символ для торговли (может быть с суффиксом, например `BTC_USDT`) |
| `exchange` | string | Биржа в верхнем регистре: `MEXC`, `BINANCE`, `UPBIT` и т.д. |
| `kind`     | string | `"spot"` или `"futures"` |
| `source`   | string | Источник события, например `"listing-detector"` |
| `reason`   | string | Причина, например `"new_listing"` |
| `at`       | number | Unix timestamp в миллисекундах |
| `mcap_usd` | number | Опционально, рыночная капитализация в USD |

**Обновление конфига (broadcast от дашборда/API):**

```json
{
  "type": "config_update",
  "data": { ... }
}
```

`data` — произвольный JSON (whitelist/blacklist, настройки бирж и т.д.). Ваш бот может игнорировать или использовать по своему протоколу.

**Ping (keepalive от Listener):**

```json
{ "type": "ping" }
```

Рекомендуется отвечать:

```json
{ "type": "pong" }
```

#### Исходящие от клиента

- **Pong** (в ответ на ping): `{"type":"pong"}`

Больше ничего от клиента не требуется; алерты только приходят.

---

## 4. HTTP API

Базовый URL: `http://<host>:8082`. На все запросы (кроме `GET /health`) нужен заголовок:

```
X-Auth-Token: <LISTENER_AUTH_TOKEN>
```

Либо токен, совпадающий с `BOT_KEY` (если он задан).

### 4.1 Эндпоинты

| Метод | Путь | Описание |
|-------|------|----------|
| GET  | `/health` | Проверка живости, без авторизации |
| GET  | `/api/status` | Статус и текущий конфиг |
| GET  | `/api/traders` | Список зарегистрированных traders (url + masked token) |
| POST | `/api/traders` | Добавить trader: `{"url":"...", "auth_token":"..."}` |
| DELETE | `/api/traders` | Удалить trader: `{"url":"..."}` |
| POST | `/api/config` | Обновить конфиг (и разослать config_update по WS) |
| POST | `/api/test` | Отправить тестовый алерт всем WS-клиентам |

### 4.2 GET /api/status

Ответ (успех):

```json
{
  "status": "ok",
  "message": "Status retrieved",
  "data": {
    "trader_count": 2,
    "ws_clients": 1,
    "config_path": "/app/listener_config.json",
    "enabled_exchanges": ["binance", "mexc"],
    "config": {
      "allow_premarket": true,
      "allow_launchpool": false,
      "allow_roadmap": false,
      "allow_alpha": false,
      "exchanges": {
        "BINANCE": { "enabled": true, "allow_spot": true, "allow_futures": false },
        "MEXC": { "enabled": true, "allow_spot": true, "allow_futures": true }
      },
      "symbols_whitelist": [],
      "symbols_blacklist": []
    }
  }
}
```

### 4.3 POST /api/test — тестовый алерт

Тело запроса:

```json
{
  "symbol": "BTC_USDT",
  "exchange": "MEXC",
  "kind": "futures",
  "source": "dashboard_test",
  "reason": "manual",
  "mcap_usd": 1000000
}
```

Сервер рассылает это сообщение всем подключённым WebSocket-клиентам в формате `type: "alert"` (как в разделе 3.2). Удобно для проверки подключения вашего бота.

### 4.4 POST /api/config — обновление конфига

Можно слать частичное обновление. Примеры:

- Глобальные флаги: `{"allow_premarket": true}`
- Одна биржа: `{"update_exchange": {"name": "Binance", "settings": {"enabled": true, "allow_spot": true, "allow_futures": false}}}`
- Списки: `{"symbols_whitelist": ["BTC"], "symbols_blacklist": []}`

После применения тот же payload уходит всем клиентам в сообщении `config_update`.

---

## 5. Как подключить свой проект

### 5.1 Минимальный клиент (псевдокод)

1. Открыть WebSocket: `ws://listener-host:8083/ws` (или с `?token=...` если задан BOT_KEY).
2. В цикле читать сообщения.
3. Парсить JSON, смотреть `type`:
   - `alert` — извлечь `symbol`, `exchange`, `kind`, `at`, `mcap_usd` и вызвать свою логику (открытие позиции, уведомление и т.д.).
   - `config_update` — при необходимости обновить локальный конфиг.
   - `ping` — отправить `{"type":"pong"}`.
4. При обрыве соединения — переподключиться с backoff.

### 5.2 Пример тела алерта для своей логики

После парсинга сообщения с `type: "alert"` у вас есть:

- `symbol` — что торговать (например `BTC_USDT` или `BTCUSDT` в зависимости от биржи).
- `exchange` — биржа (MEXC, BINANCE, UPBIT, …).
- `kind` — `spot` или `futures`.
- `at` — время события (мс).
- `mcap_usd` — опционально, для фильтрации по капитализации.

Дальше вы сами решаете: подходит ли вам эта биржа/тип рынка, проходите ли по своим whitelist/blacklist и т.д.

### 5.3 Проверка без своего кода

- Убедиться, что Listener запущен и есть хотя бы один источник листингов (NewListings.pro или ListingFeed).
- Вызвать `POST /api/test` с заголовком `X-Auth-Token` и телом из 4.3 — все подключённые WS-клиенты получат алерт.
- Подключиться к `ws://.../ws` любым WS-клиентом (например, из браузера или Postman) и убедиться, что после теста приходит сообщение с `type: "alert"`.

---

## 6. Источники листингов (на стороне Listener)

Listener сам получает события извне и после фильтрации рассылает их вам. Ваш проект с этим не взаимодействует.

- **NewListings.pro** — WebSocket, формат сообщения: `exchange`, `type` (kind), `detections: [{ "ticker": "SYMBOL" }]`. Listener принимает только `kind`: `spot` или `futures`/`future`.
- **ListingFeed** (свой сервис) — тот же формат: `exchange`, `type`, `detections`. Подключение: в конфиге Listener указать `newlistings_url: "ws://listing-feed-host:9090/ws"` (и при необходимости auth).

Конфиг Listener хранится в JSON (например `listener_config.json`): там задаются `newlistings_url`, `newlistings_auth_header`, фильтры по биржам (`exchanges`), глобальные флаги и т.д. Для интеграции своего проекта достаточно знать про WebSocket и HTTP API выше; менять конфиг можно через `POST /api/config` или вручную.

---

## 7. Краткая шпаргалка

| Задача | Действие |
|--------|----------|
| Получать алерты в своём боте | Подключиться по WebSocket к `ws://host:8083/ws` (при необходимости с `?token=BOT_KEY`), обрабатывать сообщения с `type: "alert"`. |
| Узнать конфиг и число клиентов | `GET /api/status` с заголовком `X-Auth-Token`. |
| Отправить тестовый алерт всем клиентам | `POST /api/test` с телом `{ "symbol", "exchange", "kind", "source", "reason", "mcap_usd?" }` и `X-Auth-Token`. |
| Обновить настройки и разослать клиентам | `POST /api/config` с нужным JSON и `X-Auth-Token`. |

Формат алерта на стороне клиента всегда один и тот же (раздел 3.2), независимо от того, откуда Listener получил событие (NewListings.pro, ListingFeed или тест через API).
