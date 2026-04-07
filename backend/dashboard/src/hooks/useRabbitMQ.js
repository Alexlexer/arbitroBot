import { useEffect, useState, useRef } from 'react';
import { Client } from '@stomp/stompjs';

// Bigger batch => fewer React updates and less frontend CPU.
const TICKER_BATCH_MS = 1800;
const isDev = typeof import.meta !== 'undefined' && import.meta.env?.DEV;

export const useRabbitMQ = () => {
  const [tickers, setTickers] = useState({});
  const [accountState, setAccountState] = useState(null);
  const [botConfig, setBotConfig] = useState(null);
  const [listenerAlert, setListenerAlert] = useState(null);
  const [listenerOpportunity, setListenerOpportunity] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const clientRef = useRef(null);
  const authCallbacksRef = useRef({});
  const tickerBatchRef = useRef({});
  const batchTimerRef = useRef(null);
  const exchangeSubsRef = useRef(new Map()); // exName -> stomp subscription

  const flushTickerBatch = () => {
    const batch = tickerBatchRef.current;
    if (!batch || Object.keys(batch).length === 0) return;
    tickerBatchRef.current = {};
    setTickers((prev) => ({ ...prev, ...batch }));
  };

  const onTickerMessage = (message) => {
    const ticker = JSON.parse(message.body);
    const key = `${ticker.exchange}-${ticker.symbol}`;
    tickerBatchRef.current[key] = ticker;
    if (!batchTimerRef.current) {
      batchTimerRef.current = setInterval(flushTickerBatch, TICKER_BATCH_MS);
    }
  };

  const sendBotCommand = (command) => {
    if (clientRef.current && isConnected) {
      clientRef.current.publish({
        destination: '/exchange/arbit_hub/bot.commands',
        body: JSON.stringify(command),
      });
    }
  };

  const AUTH_TIMEOUT_MS = 15000; // 15s (registration does Argon2 hashing)

  /** Returns Promise<{ ok, username?, error? }>. Rejects on timeout or not connected. */
  const login = (username, password) => {
    return new Promise((resolve, reject) => {
      if (!clientRef.current || !isConnected) {
        reject(new Error('Not connected to broker'));
        return;
      }
      const requestId = crypto.randomUUID?.() || `req-${Date.now()}`;
      authCallbacksRef.current[requestId] = { resolve };
      sendBotCommand({ type: 'dashboard_login', username, password, request_id: requestId });
      setTimeout(() => {
        if (authCallbacksRef.current[requestId]) {
          delete authCallbacksRef.current[requestId];
          reject(new Error('Login timeout'));
        }
      }, AUTH_TIMEOUT_MS);
    });
  };

  /** Returns Promise<{ ok, username?, error? }>. Rejects on timeout or not connected. */
  const register = (username, password, inviteCode) => {
    return new Promise((resolve, reject) => {
      if (!clientRef.current || !isConnected) {
        reject(new Error('Not connected to broker'));
        return;
      }
      const requestId = crypto.randomUUID?.() || `req-${Date.now()}`;
      authCallbacksRef.current[requestId] = { resolve };
      sendBotCommand({
        type: 'dashboard_register',
        username,
        password,
        invite_code: inviteCode,
        request_id: requestId,
      });
      setTimeout(() => {
        if (authCallbacksRef.current[requestId]) {
          delete authCallbacksRef.current[requestId];
          reject(new Error('Registration timeout'));
        }
      }, AUTH_TIMEOUT_MS);
    });
  };

  // Keep tickers in state close to opportunities freshness window.
  // (dashboard opportunity logic uses FRESH_MS=90s)
  const TICKER_RETAIN_MS = 95 * 1000;
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      setTickers((prev) => {
        const fresh = {};
        let changed = false;
        Object.entries(prev).forEach(([key, ticker]) => {
          if (now - ticker.timestamp < TICKER_RETAIN_MS) {
            fresh[key] = ticker;
          } else {
            changed = true;
          }
        });
        return changed ? fresh : prev;
      });
    }, 30000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    // In production nginx proxies /ws → rabbitmq:15674/ws internally.
    // In dev Vite proxies /ws → localhost:15674/ws.
    // Always connect through the same origin so the proxy handles routing.
    const envUrl = typeof import.meta !== 'undefined' && import.meta.env?.VITE_RABBITMQ_WS_URL;
    const hostname = typeof window !== 'undefined' ? window.location.hostname : '127.0.0.1';
    const isHttps = typeof window !== 'undefined' && window.location.protocol === 'https:';
    const scheme = isHttps ? 'wss' : 'ws';
    const originPort = typeof window !== 'undefined' ? window.location.port : '5174';
    const brokerURL = (envUrl && envUrl.trim())
      ? envUrl.trim()
      : `${scheme}://${hostname}${originPort ? ':' + originPort : ''}/ws`;
    const client = new Client({
      brokerURL,
      connectHeaders: {
        login: 'guest',
        passcode: 'guest',
      },
      // Avoid heavy console spam in production; keep only error logs.
      debug: isDev ? (str) => console.log('STOMP:', str) : () => { },
      reconnectDelay: 5000,
      heartbeatIncoming: 4000,
      heartbeatOutgoing: 4000,
      onWebSocketOpen: () => {
        console.log('WebSocket opened:', brokerURL);
      },
      onWebSocketClose: (evt) => {
        const code = evt?.code;
        const reason = evt?.reason;
        console.error('WebSocket closed:', { code, reason, url: brokerURL });
      },
    });

    client.onConnect = () => {
      if (isDev) console.log('Connected to WebStomp');
      setIsConnected(true);
      // Subscribe to bulk ticker snapshot — bot sends all tickers as one message per interval tick.
      client.subscribe('/exchange/arbit_hub/tickers.snapshot', (message) => {
        try {
          const snapshot = JSON.parse(message.body);
          if (!Array.isArray(snapshot)) return;
          const batch = {};
          for (const ticker of snapshot) {
            if (!ticker?.symbol || !ticker?.exchange) continue;
            batch[`${ticker.exchange}-${ticker.symbol}`] = ticker;
          }
          tickerBatchRef.current = { ...tickerBatchRef.current, ...batch };
          if (!batchTimerRef.current) {
            batchTimerRef.current = setInterval(flushTickerBatch, TICKER_BATCH_MS);
          }
        } catch { /* ignore malformed */ }
      });

      // Subscribe to account state
      client.subscribe('/exchange/arbit_hub/account.state', (message) => {
        try { setAccountState(JSON.parse(message.body)); } catch { }
      });

      client.subscribe('/exchange/arbit_hub/bot.config', (message) => {
        setBotConfig(JSON.parse(message.body));
      });

      client.subscribe('/exchange/arbit_hub/listener.alert', (message) => {
        setListenerAlert(JSON.parse(message.body));
      });

      client.subscribe('/exchange/arbit_hub/listener.opportunity', (message) => {
        setListenerOpportunity(JSON.parse(message.body));
      });

      // Subscribe to dashboard auth replies
      client.subscribe('/exchange/arbit_hub/dashboard.auth', (message) => {
        const body = JSON.parse(message.body);
        const { ok, request_id, username, error } = body;
        const cb = authCallbacksRef.current[request_id];
        if (cb) {
          delete authCallbacksRef.current[request_id];
          cb.resolve({ ok, username: username || null, error: error || null });
        }
      });
    };

    client.onStompError = (frame) => {
      console.error('Broker reported error: ' + frame.headers['message']);
      console.error('Additional details: ' + frame.body);
    };

    client.onWebSocketError = (event) => {
      console.error('WebSocket Error', {
        url: event?.target?.url,
        readyState: event?.target?.readyState,
        eventType: event?.type,
      });
    };

    client.activate();
    clientRef.current = client;

    return () => {
      if (batchTimerRef.current) clearInterval(batchTimerRef.current);
      batchTimerRef.current = null;
      tickerBatchRef.current = {};
      client.deactivate();
    };
  }, []);

  return { tickers, accountState, botConfig, listenerAlert, listenerOpportunity, isConnected, sendBotCommand };
};
