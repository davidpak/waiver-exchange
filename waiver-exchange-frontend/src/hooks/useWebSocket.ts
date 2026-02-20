'use client';

import { apiClient } from '@/lib/api-client';
import { useAuthStore } from '@/stores/authStore';
import type { OrderPlaceRequest, OrderPlaceResponse } from '@/types/api';
import { useCallback, useEffect, useRef, useState } from 'react';

interface UseWebSocketReturn {
  connected: boolean;
  authenticated: boolean;
  placeOrder: (order: OrderPlaceRequest) => Promise<OrderPlaceResponse>;
  getPositions: () => Promise<any>;
  getTrades: () => Promise<any>;
}

export function useWebSocket(): UseWebSocketReturn {
  const { isAuthenticated, token, setWebSocketState } = useAuthStore();
  const [connected, setConnected] = useState(false);
  const [authenticated, setAuthenticated] = useState(false);
  const reconnectTimer = useRef<NodeJS.Timeout | null>(null);
  const mountedRef = useRef(true);

  const connect = useCallback(async () => {
    if (!isAuthenticated || !token) return;

    try {
      await apiClient.connectWebSocket();
      if (!mountedRef.current) return;
      setConnected(true);
      setWebSocketState(true, false);

      // Authenticate
      const authResult = await apiClient.ws.authenticate(token, '');
      if (!mountedRef.current) return;
      if (authResult.authenticated) {
        setAuthenticated(true);
        setWebSocketState(true, true);
      }
    } catch {
      if (!mountedRef.current) return;
      setConnected(false);
      setAuthenticated(false);
      setWebSocketState(false, false);

      // Reconnect after 3s
      reconnectTimer.current = setTimeout(connect, 3000);
    }
  }, [isAuthenticated, token, setWebSocketState]);

  useEffect(() => {
    mountedRef.current = true;
    connect();

    return () => {
      mountedRef.current = false;
      if (reconnectTimer.current) clearTimeout(reconnectTimer.current);
      apiClient.disconnectWebSocket();
    };
  }, [connect]);

  const placeOrder = useCallback(
    async (order: OrderPlaceRequest): Promise<OrderPlaceResponse> => {
      if (!connected || !authenticated) {
        throw new Error('WebSocket not connected or authenticated');
      }
      return apiClient.ws.placeOrder(order);
    },
    [connected, authenticated]
  );

  const getPositions = useCallback(async () => {
    if (!connected || !authenticated) throw new Error('Not connected');
    return apiClient.ws.getAccountPositions();
  }, [connected, authenticated]);

  const getTrades = useCallback(async () => {
    if (!connected || !authenticated) throw new Error('Not connected');
    return apiClient.ws.getAccountTrades();
  }, [connected, authenticated]);

  return { connected, authenticated, placeOrder, getPositions, getTrades };
}
