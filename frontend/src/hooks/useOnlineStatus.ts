/**
 * Hook for detecting online/offline status and triggering background sync.
 * Listens to browser online/offline events and replays queued actions
 * when connectivity is restored.
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

export function useOnlineStatus(): boolean {
  const isOnline = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);

  // Added: Auto-replay queued actions when coming back online
  useEffect(() => {
    if (isOnline) {
      backgroundSync.processPending().then(({ processed, failed }) => {
        if (processed > 0) {
          console.info(`Background sync: ${processed} actions replayed, ${failed} failed`);
        }
      });
    }
  }, [isOnline]);

  return isOnline;
}
