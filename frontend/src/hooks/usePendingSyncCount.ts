/**
 * usePendingSyncCount (TMAIL-88)
 *
 * Reactive count of queued background-sync actions. Subscribes to the
 * backgroundSync pub/sub and re-renders the consumer whenever the queue
 * changes (enqueue, replay, retry, clear).
 *
 * Used by PendingSyncBanner; safe for any component that wants to surface
 * "you have N offline edits waiting" UX.
 */
import { useEffect, useState } from 'react';
import { backgroundSync } from '../utils/background-sync';

export function usePendingSyncCount(): number {
  const [count, setCount] = useState(0);

  useEffect(() => {
    let cancelled = false;

    const refresh = () => {
      backgroundSync.getPendingCount().then((n) => {
        if (!cancelled) setCount(n);
      }).catch(() => {
        // NOTE: IndexedDB can be unavailable (private browsing, quota errors).
        // Treat as "no pending" — silent so we don't flood the console.
        if (!cancelled) setCount(0);
      });
    };

    refresh();
    const unsubscribe = backgroundSync.subscribe(refresh);
    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, []);

  return count;
}
