import { useEffect, useState, useRef } from 'react';
import { Client } from '@stomp/stompjs';

export const useRabbitMQ = () => {
  const [tickers, setTickers] = useState({});
  const [accountState, setAccountState] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const clientRef = useRef(null);

  // Cleanup stale tickers every 10s
  useEffect(() => {
    const interval = setInterval(() => {
      const now = Date.now();
      setTickers((prev) => {
        const fresh = {};
        let changed = false;
        Object.entries(prev).forEach(([key, ticker]) => {
          if (now - ticker.timestamp < 60000) {
            fresh[key] = ticker;
          } else {
            changed = true;
          }
        });
        return changed ? fresh : prev;
      });
    }, 10000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    const client = new Client({
      brokerURL: 'ws://127.0.0.1:15674/ws',
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

  return { tickers, accountState, isConnected };
};
