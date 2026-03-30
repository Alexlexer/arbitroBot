import { useEffect, useState, useRef } from 'react';
import { Client } from '@stomp/stompjs';

export const useRabbitMQ = () => {
  const [tickers, setTickers] = useState({});
  const [accountState, setAccountState] = useState(null);
  const [isConnected, setIsConnected] = useState(false);
  const clientRef = useRef(null);

  useEffect(() => {
    const client = new Client({
      brokerURL: 'ws://localhost:15674/ws',
      connectHeaders: {
        login: 'guest',
        passcode: 'guest',
      },
      debug: (str) => {
        // console.log(str);
      },
      reconnectDelay: 5000,
      heartbeatIncoming: 4000,
      heartbeatOutgoing: 4000,
    });

    client.onConnect = () => {
      setIsConnected(true);
      // Subscribe to all tickers
      client.subscribe('/exchange/arbit_hub/ticker.*', (message) => {
        const ticker = JSON.parse(message.body);
        setTickers((prev) => ({
          ...prev,
          [`${ticker.exchange}-${ticker.symbol}`]: ticker,
        }));
      });

      // Subscribe to account state
      client.subscribe('/exchange/arbit_hub/account.state', (message) => {
        setAccountState(JSON.parse(message.body));
      });
    };

    client.onDisconnect = () => {
      setIsConnected(false);
    };

    client.activate();
    clientRef.current = client;

    return () => {
      client.deactivate();
    };
  }, []);

  const sendCommand = (type, payload) => {
    if (clientRef.current && isConnected) {
      clientRef.current.publish({
        destination: '/exchange/arbit_hub/commands',
        body: JSON.stringify({ type, payload }),
      });
    }
  };

  return { tickers, accountState, isConnected, sendCommand };
};
