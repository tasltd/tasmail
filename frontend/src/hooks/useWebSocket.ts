import { useEffect, useRef, useCallback, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { WS_URL } from '../utils/constants';

export interface WsEvent {
  type: 'new_mail' | 'unread_update' | 'quota_update' | 'ping' | 'error';
  folder?: string;
  count?: number;
  unread?: number;
  used_bytes?: number;
  total_bytes?: number;
  timestamp?: number;
  message?: string;
}

interface UseWebSocketOptions {
  /** JWT access token for authentication */
  token: string | null;
  /** Called when a new event is received */
  onEvent?: (event: WsEvent) => void;
  /** Reconnect delay in ms (default: 3000) */
  reconnectDelay?: number;
}

/**
 * Hook for real-time push notifications via WebSocket.
 * Automatically reconnects on disconnect and invalidates
 * relevant React Query caches when new mail events arrive.
 */
export function useWebSocket({ token, onEvent, reconnectDelay = 3000 }: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const queryClient = useQueryClient();
  const [connected, setConnected] = useState(false);

  const connect = useCallback(() => {
    if (!token) return;

    // Clean up existing connection
    if (wsRef.current) {
      wsRef.current.close();
    }

    const url = `${WS_URL}?token=${encodeURIComponent(token)}`;
    const ws = new WebSocket(url);

    ws.onopen = () => {
      setConnected(true);
      // Subscribe to INBOX by default
      ws.send('subscribe:INBOX');
    };

    ws.onmessage = (event) => {
      try {
        const data: WsEvent = JSON.parse(event.data);
        onEvent?.(data);

        // Added: auto-invalidate React Query caches based on event type
        if (data.type === 'new_mail' || data.type === 'unread_update') {
          queryClient.invalidateQueries({ queryKey: ['folders'] });
          if (data.folder) {
            queryClient.invalidateQueries({ queryKey: ['messages', data.folder] });
          }
        }
        if (data.type === 'quota_update') {
          queryClient.invalidateQueries({ queryKey: ['quota'] });
        }
      } catch {
        // Ignore non-JSON messages
      }
    };

    ws.onclose = () => {
      setConnected(false);
      // Reconnect after delay
      reconnectTimerRef.current = setTimeout(connect, reconnectDelay);
    };

    ws.onerror = () => {
      ws.close();
    };

    wsRef.current = ws;
  }, [token, onEvent, reconnectDelay, queryClient]);

  // Subscribe to a specific folder
  const subscribe = useCallback((folder: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(`subscribe:${folder}`);
    }
  }, []);

  useEffect(() => {
    connect();
    return () => {
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
      }
      if (wsRef.current) {
        wsRef.current.close();
      }
    };
  }, [connect]);

  return { connected, subscribe };
}
