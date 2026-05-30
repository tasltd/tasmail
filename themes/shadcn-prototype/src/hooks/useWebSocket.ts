// TMAIL-328: Modern UI real-time push subscription.
//
// Opens a WebSocket against `${WS_URL}?token=<jwt>` (matches the backend
// contract in backend/src/handlers/websocket.rs — token is a query param
// because the WS upgrade handshake can't carry an Authorization header).
//
// On open we send `subscribe:<folder>` so the backend spins up an IMAP IDLE
// bridge for that folder (handlers/websocket.rs::handle_subscribe). Incoming
// frames are JSON `WsEvent`s; we map each event type onto a React Query cache
// invalidation so the sidebar folder counts and message list refresh
// automatically when mail lands without polling.
//
// Reconnect strategy: exponential backoff capped at 30s. The fixed-3s retry
// the classic SPA hook (frontend/src/hooks/useWebSocket.ts) uses hammers the
// server when it's down or the network is flapping — bounding the cap and
// adding jitter is the standard fix.
import { useCallback, useEffect, useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { WS_URL } from '@/api/constants';

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
  /** JWT access token. When null/undefined the hook stays idle. */
  token: string | null | undefined;
  /** Folder name(s) to auto-subscribe to on connect. Defaults to ['INBOX']. */
  subscribeFolders?: string[];
  /** Optional consumer callback for raw events (testing / diagnostics). */
  onEvent?: (event: WsEvent) => void;
  /** Initial reconnect delay in ms (doubled per attempt). Default 1000. */
  initialReconnectDelayMs?: number;
  /** Hard cap on reconnect delay in ms. Default 30000. */
  maxReconnectDelayMs?: number;
}

// Exported for unit-test access. Plain pure helper so the test doesn't need
// to drive the full hook through fake timers — given an attempt count it
// returns the next delay in ms (jittered).
export function nextBackoffDelay(
  attempt: number,
  initialMs: number,
  capMs: number,
): number {
  // attempt is 0-based: 0 → initialMs, 1 → 2*initialMs, 2 → 4*initialMs …
  const exp = Math.min(capMs, initialMs * Math.pow(2, attempt));
  // ±25% jitter so a fleet of reconnecting clients doesn't synchronise.
  const jitter = exp * 0.25 * (Math.random() * 2 - 1);
  return Math.max(0, Math.round(exp + jitter));
}

/**
 * Hook for the Modern UI WebSocket push channel.
 *
 * Returns `{ connected, subscribe }`. Consumers usually only need to render
 * `connected` as a status indicator; `subscribe` is exposed so a future
 * "watch this folder too" UX can ask the backend to add an IDLE bridge for
 * another folder beyond INBOX without tearing down the socket.
 */
export function useWebSocket({
  token,
  subscribeFolders,
  onEvent,
  initialReconnectDelayMs = 1000,
  maxReconnectDelayMs = 30_000,
}: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attemptRef = useRef(0);
  // Capture the consumer callback in a ref so changing identity doesn't
  // tear the socket down on every parent re-render.
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;
  // Same trick for the auto-subscribe folder list. Memoising at the call
  // site is awkward (string arrays), so we just snapshot the latest value.
  const subscribeFoldersRef = useRef<string[]>(subscribeFolders ?? ['INBOX']);
  subscribeFoldersRef.current = subscribeFolders ?? ['INBOX'];

  const queryClient = useQueryClient();
  const [connected, setConnected] = useState(false);

  const connect = useCallback(() => {
    if (!token) return;

    // Tear down any prior socket so we don't leak handlers.
    if (wsRef.current) {
      try {
        wsRef.current.close();
      } catch {
        // Already closed — ignore.
      }
      wsRef.current = null;
    }

    const url = `${WS_URL}?token=${encodeURIComponent(token)}`;
    let ws: WebSocket;
    try {
      ws = new WebSocket(url);
    } catch (e) {
      // Browsers throw synchronously for malformed URLs / blocked origins.
      // Schedule a retry so a transient config issue doesn't strand the UI.
      const delay = nextBackoffDelay(
        attemptRef.current,
        initialReconnectDelayMs,
        maxReconnectDelayMs,
      );
      attemptRef.current += 1;
      reconnectTimerRef.current = setTimeout(connect, delay);
      // eslint-disable-next-line no-console
      console.warn('[useWebSocket] failed to open socket, retrying', e);
      return;
    }

    ws.onopen = () => {
      attemptRef.current = 0; // reset backoff on successful open
      setConnected(true);
      for (const folder of subscribeFoldersRef.current) {
        try {
          ws.send(`subscribe:${folder}`);
        } catch {
          // Send can throw if the socket flipped state between open + send.
          // Fall through — the close handler will schedule a reconnect.
        }
      }
    };

    ws.onmessage = (event) => {
      let data: WsEvent;
      try {
        data = JSON.parse(event.data) as WsEvent;
      } catch {
        // Non-JSON frames (e.g. heartbeat text). Drop silently.
        return;
      }

      onEventRef.current?.(data);

      // Cache invalidations — keep aligned with the route-keyed query keys
      // the rest of the Modern UI uses (EmailClient.tsx wires ['folders'],
      // ['messages', folder], etc.).
      if (data.type === 'new_mail' || data.type === 'unread_update') {
        queryClient.invalidateQueries({ queryKey: ['folders'] });
        if (data.folder) {
          queryClient.invalidateQueries({
            queryKey: ['messages', data.folder],
          });
        }
      } else if (data.type === 'quota_update') {
        queryClient.invalidateQueries({ queryKey: ['quota'] });
      }
      // 'ping' and 'error' fall through — Ping is just a heartbeat we don't
      // need to act on, Error is surfaced via the onEvent callback for the
      // consumer to render.
    };

    ws.onclose = () => {
      setConnected(false);
      wsRef.current = null;
      // Only reconnect if we still have a token. Logout sets token to null,
      // which then short-circuits this branch via the !token guard inside
      // connect() — the next open attempt won't fire until login happens.
      if (!token) return;
      const delay = nextBackoffDelay(
        attemptRef.current,
        initialReconnectDelayMs,
        maxReconnectDelayMs,
      );
      attemptRef.current += 1;
      reconnectTimerRef.current = setTimeout(connect, delay);
    };

    ws.onerror = () => {
      // Force a close so onclose can drive the reconnect timer. Browsers
      // sometimes fire onerror without onclose for handshake-stage failures.
      try {
        ws.close();
      } catch {
        // Already closed — ignore.
      }
    };

    wsRef.current = ws;
  }, [token, queryClient, initialReconnectDelayMs, maxReconnectDelayMs]);

  // Imperative subscribe — lets a future "watch another folder" UX request
  // an IDLE bridge for a folder beyond the initial set without reconnecting.
  const subscribe = useCallback((folder: string) => {
    const ws = wsRef.current;
    if (ws?.readyState === WebSocket.OPEN) {
      try {
        ws.send(`subscribe:${folder}`);
      } catch {
        // Best effort — caller can retry once `connected` flips true again.
      }
    }
  }, []);

  useEffect(() => {
    connect();
    return () => {
      // Cancel any pending reconnect so unmount doesn't resurrect the socket.
      if (reconnectTimerRef.current) {
        clearTimeout(reconnectTimerRef.current);
        reconnectTimerRef.current = null;
      }
      if (wsRef.current) {
        try {
          wsRef.current.close();
        } catch {
          // Already closed — ignore.
        }
        wsRef.current = null;
      }
      attemptRef.current = 0;
      setConnected(false);
    };
  }, [connect]);

  return { connected, subscribe };
}
