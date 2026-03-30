import { useEffect, useState, useRef, useCallback } from 'react';
import * as signalR from '@microsoft/signalr';

// Batch ticker state updates to reduce React re-renders (same cadence as old STOMP hook).
const TICKER_BATCH_MS = 1800;
// Evict tickers older than this (opportunities.js uses 90s freshness window).
const TICKER_RETAIN_MS = 95_000;

const isDev = typeof import.meta !== 'undefined' && import.meta.env?.DEV;

// ---------------------------------------------------------------------------
// State-message parser
// The Rust bot publishes a JSON state blob every ~2s.  The exact shape is:
//   Full state:   { tickers: { "Binance-BTC": ticker, ... }, account: {...} }
//   Single ticker (arbit_hub.ticker.*): { symbol, exchange, timestamp, bids, asks }
//   Account only: { total_equity_usdt, exchange_states, ... }
// We also handle a nested ticker map: { "Binance": { "BTC": ticker } }
// ---------------------------------------------------------------------------

function extractTickers(tickersData) {
  const out = {};
  if (!tickersData || typeof tickersData !== 'object') return out;
  Object.entries(tickersData).forEach(([k, v]) => {
    if (!v) return;
    if (v.symbol && v.exchange) {
      // Flat: { "Binance-BTC": { symbol, exchange, ... } }
      out[`${v.exchange}-${v.symbol}`] = v;
    } else if (typeof v === 'object') {
      // Nested: { "Binance": { "BTC": ticker } }
      Object.values(v).forEach((ticker) => {
        if (ticker?.symbol && ticker?.exchange) {
          out[`${ticker.exchange}-${ticker.symbol}`] = ticker;
        }
      });
    }
  });
  return out;
}

// ---------------------------------------------------------------------------

export function useSignalR() {
  const [tickers, setTickers]                   = useState({});
  const [accountState, setAccountState]         = useState(null);
  const [botConfig, setBotConfig]               = useState(null);
  const [listenerAlert, setListenerAlert]       = useState(null);
  const [listenerOpportunity, setListenerOpportunity] = useState(null);
  const [isConnected, setIsConnected]           = useState(false);

  const connectionRef   = useRef(null);
  const tickerBatchRef  = useRef({});
  const batchTimerRef   = useRef(null);

  // ── Ticker batching ───────────────────────────────────────────────────────

  const startBatchFlush = useCallback(() => {
    if (batchTimerRef.current) return;
    batchTimerRef.current = setInterval(() => {
      const batch = tickerBatchRef.current;
      if (Object.keys(batch).length === 0) return;
      tickerBatchRef.current = {};
      setTickers((prev) => ({ ...prev, ...batch }));
    }, TICKER_BATCH_MS);
  }, []);

  const queueTicker = useCallback((key, ticker) => {
    tickerBatchRef.current[key] = ticker;
    startBatchFlush();
  }, [startBatchFlush]);

  // ── State message router ──────────────────────────────────────────────────

  const handleReceiveState = useCallback((state) => {
    if (!state || typeof state !== 'object') return;

    // Individual ticker (routing key arbit_hub.ticker.BTC)
    if (state.symbol && state.exchange) {
      queueTicker(`${state.exchange}-${state.symbol}`, state);
      return;
    }

    // Full state blob
    if (state.tickers) {
      const flat = extractTickers(state.tickers);
      Object.entries(flat).forEach(([k, t]) => queueTicker(k, t));
    }
    if (state.account) {
      setAccountState(state.account);
    }

    // Bare account state (routing key account.state)
    if (state.total_equity_usdt !== undefined || state.exchange_states) {
      setAccountState(state);
    }
  }, [queueTicker]);

  // ── Stale ticker eviction ─────────────────────────────────────────────────

  useEffect(() => {
    const interval = setInterval(() => {
      const cutoff = Date.now() - TICKER_RETAIN_MS;
      setTickers((prev) => {
        let changed = false;
        const next = {};
        Object.entries(prev).forEach(([k, t]) => {
          if (t.timestamp >= cutoff) {
            next[k] = t;
          } else {
            changed = true;
          }
        });
        return changed ? next : prev;
      });
    }, 30_000);
    return () => clearInterval(interval);
  }, []);

  // ── Send bot command ──────────────────────────────────────────────────────

  const sendBotCommand = useCallback(async (command) => {
    const conn = connectionRef.current;
    if (!conn || conn.state !== signalR.HubConnectionState.Connected) {
      isDev && console.warn('[SignalR] sendBotCommand: not connected');
      return;
    }
    try {
      await conn.invoke('SendBotCommand', command);
    } catch (err) {
      console.error('[SignalR] SendBotCommand error:', err);
    }
  }, []);

  // ── Hub connection ────────────────────────────────────────────────────────

  useEffect(() => {
    const connection = new signalR.HubConnectionBuilder()
      .withUrl('/hubs/arbit', {
        // JWT is read lazily so it picks up the token set during Login
        accessTokenFactory: () => localStorage.getItem('arbit_token') ?? '',
        // Prefer WebSockets, fall back to LongPolling (SSE blocked by some proxies)
        transport: signalR.HttpTransportType.WebSockets | signalR.HttpTransportType.LongPolling,
      })
      .withAutomaticReconnect([1_000, 2_000, 5_000, 10_000, 30_000])
      .configureLogging(isDev ? signalR.LogLevel.Information : signalR.LogLevel.Warning)
      .build();

    connectionRef.current = connection;

    // ── Server → client handlers ──────────────────────────────────────────
    connection.on('ReceiveState', handleReceiveState);

    connection.on('ReceiveBotConfig', (config) => {
      isDev && console.log('[SignalR] ReceiveBotConfig', config);
      setBotConfig(config);
    });

    connection.on('ReceiveListenerAlert', (alert) => {
      isDev && console.log('[SignalR] ReceiveListenerAlert', alert);
      setListenerAlert(alert);
    });

    connection.on('ReceiveListenerOpportunity', (opp) => {
      isDev && console.log('[SignalR] ReceiveListenerOpportunity', opp);
      setListenerOpportunity(opp);
    });

    connection.on('ReceiveConnectionStatus', (connected) => {
      isDev && console.log('[SignalR] RabbitMQ status:', connected);
      setIsConnected(connected);
    });

    // ── Lifecycle ─────────────────────────────────────────────────────────
    connection.onreconnecting(() => {
      isDev && console.log('[SignalR] Reconnecting…');
      setIsConnected(false);
    });

    connection.onreconnected(() => {
      isDev && console.log('[SignalR] Reconnected');
      // ReceiveConnectionStatus from server will set isConnected accurately
    });

    connection.onclose((err) => {
      if (err) console.error('[SignalR] Connection closed with error:', err);
      else isDev && console.log('[SignalR] Connection closed');
      setIsConnected(false);
    });

    async function start() {
      try {
        await connection.start();
        isDev && console.log('[SignalR] Connected to /hubs/arbit');
      } catch (err) {
        // If the token is missing/invalid the server returns 401 —
        // the component that owns auth state should redirect to Login.
        console.error('[SignalR] Start failed:', err);
      }
    }

    start();

    return () => {
      if (batchTimerRef.current) {
        clearInterval(batchTimerRef.current);
        batchTimerRef.current = null;
      }
      tickerBatchRef.current = {};
      connection.stop();
    };
  }, [handleReceiveState]);

  return {
    tickers,
    accountState,
    botConfig,
    listenerAlert,
    listenerOpportunity,
    isConnected,
    sendBotCommand,
  };
}
