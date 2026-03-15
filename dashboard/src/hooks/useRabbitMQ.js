import { useEffect, useState, useRef } from 'react';
import { Client } from '@stomp/stompjs';

export const useRabbitMQ = () => {
  const [tickers, setTickers] = useState({});
  const [accountState, setAccountState] = useState(null);
  const [botConfig, setBotConfig] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const clientRef = useRef(null);
  const authCallbacksRef = useRef({});

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

  // Keep tickers in state for 5 minutes; cleanup run every 30s
  const TICKER_RETAIN_MS = 5 * 60 * 1000;
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
    // Use same host as the page (so it works when opening http://<machine>:5173)
    const host = typeof window !== 'undefined' ? window.location.hostname : '127.0.0.1';
    const brokerURL = `ws://${host}:15674/ws`;
    const client = new Client({
      brokerURL,
      connectHeaders: {
        login: 'guest',
        passcode: 'guest',
      },
      debug: (str) => {
        console.log('STOMP Debug:', str);
      },
      reconnectDelay: 5000,
      heartbeatIncoming: 4000,
      heartbeatOutgoing: 4000,
    });

    client.onConnect = () => {
      console.log('Connected to WebStomp');
      setIsConnected(true);
      
      // Subscribe to all tickers
      // Format: /exchange/arbit_hub/ticker.<exchange>
      client.subscribe('/exchange/arbit_hub/ticker.*', (message) => {
        const ticker = JSON.parse(message.body);
        console.log('Ticker received:', ticker.symbol, ticker.exchange);
        setTickers((prev) => ({
          ...prev,
          [`${ticker.exchange}-${ticker.symbol}`]: ticker,
        }));
      });

      // Subscribe to account state
      client.subscribe('/exchange/arbit_hub/account.state', (message) => {
        console.log('Account state received');
        setAccountState(JSON.parse(message.body));
      });

      // Subscribe to bot config
      client.subscribe('/exchange/arbit_hub/bot.config', (message) => {
        console.log('Bot config received');
        setBotConfig(JSON.parse(message.body));
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
      console.error('WebSocket Error', event);
    };

    client.activate();
    clientRef.current = client;

    return () => {
      client.deactivate();
    };
  }, []);

  return { tickers, accountState, botConfig, isConnected, sendBotCommand, login, register };
};
