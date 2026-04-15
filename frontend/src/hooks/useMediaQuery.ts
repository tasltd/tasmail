// Added: Custom hook for responsive design media query detection (TMAIL-33)
import { useSyncExternalStore, useCallback } from 'react';

/**
 * Hook that tracks whether a CSS media query matches.
 * Uses window.matchMedia API with useSyncExternalStore for
 * tear-free reads during concurrent rendering.
 */
export function useMediaQuery(query: string): boolean {
  // NOTE: Subscribe to matchMedia change events
  const subscribe = useCallback(
    (callback: () => void): (() => void) => {
      const mql = window.matchMedia(query);
      mql.addEventListener('change', callback);
      return () => mql.removeEventListener('change', callback);
    },
    [query],
  );

  const getSnapshot = useCallback((): boolean => {
    return window.matchMedia(query).matches;
  }, [query]);

  // NOTE: Server snapshot always returns false (no window on server)
  const getServerSnapshot = useCallback((): boolean => {
    return false;
  }, []);

  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}
