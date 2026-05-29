/**
 * Hook for detecting online/offline status and triggering background sync.
 * Listens to browser online/offline events and replays queued actions
 * when connectivity is restored.
 *
 * Periodic-sync fallback (TMAIL-88): Firefox and Safari do not implement the
 * Workbox SyncManager API. To guarantee replay on those browsers, we also run
 * a periodic interval (every PERIODIC_SYNC_MS) that calls processPending()
 * whenever the tab is online. Chrome-family browsers still get the immediate
 * 'online' event replay on top of the periodic tick.
 */
import { useEffect, useSyncExternalStore } from 'react';
import { backgroundSync } from '../utils/background-sync';

// Added: Subscribe to browser online/offline events via useSyncExternalStore
function subscribe(callback: () => void): () => void {
  window.addEventListener('online', callback);
  window.addEventListener('offline', callback);
  return () => {
    window.removeEventListener('online', callback);
    window.removeEventListener('offline', callback);
  };
}

function getSnapshot(): boolean {
  return navigator.onLine;
}

function getServerSnapshot(): boolean {
  return true;
}

// Added (TMAIL-88): Periodic fallback interval — 30s matches Workbox default.
// Exported for the test suite so it can fast-forward the fake timer without
// having to wait the real interval.
export const PERIODIC_SYNC_MS = 30_000;

async function replay(): Promise<void> {
  try {
    const { processed, failed } = await backgroundSync.processPending();
    if (processed > 0) {
      console.info(`Background sync: ${processed} actions replayed, ${failed} failed`);
    }
  } catch (err) {
    // NOTE: IndexedDB can be unavailable (private browsing, jsdom, quota).
    // Replay is best-effort — swallowing here keeps the periodic interval
    // alive and prevents unhandled rejections in tests/SSR.
    console.warn('backgroundSync.processPending failed', err);
  }
}

export function useOnlineStatus(): boolean {
  const isOnline = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);

  // Added: Auto-replay queued actions when coming back online (instant trigger)
  useEffect(() => {
    if (isOnline) {
      void replay();
    }
  }, [isOnline]);

  // Added (TMAIL-88): Periodic fallback for browsers without the BackgroundSync
  // API (Firefox, Safari). Runs only while online so we don't burn CPU on a
  // disconnected tab.
  useEffect(() => {
    if (!isOnline) return;
    const id = window.setInterval(() => {
      void replay();
    }, PERIODIC_SYNC_MS);
    return () => window.clearInterval(id);
  }, [isOnline]);

  return isOnline;
}
