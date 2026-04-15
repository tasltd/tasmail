import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useMediaQuery } from './useMediaQuery';

describe('useMediaQuery', () => {
  let listeners: Map<string, Set<() => void>>;
  let matchState: Map<string, boolean>;

  beforeEach(() => {
    listeners = new Map();
    matchState = new Map();

    // Added: Mock window.matchMedia for controlled testing
    vi.stubGlobal('matchMedia', (query: string) => {
      if (!listeners.has(query)) {
        listeners.set(query, new Set());
      }
      return {
        matches: matchState.get(query) ?? false,
        media: query,
        addEventListener: (_event: string, cb: () => void) => {
          listeners.get(query)!.add(cb);
        },
        removeEventListener: (_event: string, cb: () => void) => {
          listeners.get(query)!.delete(cb);
        },
      };
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('returns false when media query does not match', () => {
    matchState.set('(max-width: 767px)', false);
    const { result } = renderHook(() => useMediaQuery('(max-width: 767px)'));
    expect(result.current).toBe(false);
  });

  it('returns true when media query matches', () => {
    matchState.set('(max-width: 767px)', true);
    const { result } = renderHook(() => useMediaQuery('(max-width: 767px)'));
    expect(result.current).toBe(true);
  });

  it('updates when media query match state changes', () => {
    matchState.set('(max-width: 767px)', false);
    const { result } = renderHook(() => useMediaQuery('(max-width: 767px)'));
    expect(result.current).toBe(false);

    // Changed: Simulate viewport change by updating match state and notifying listeners
    act(() => {
      matchState.set('(max-width: 767px)', true);
      const cbs = listeners.get('(max-width: 767px)');
      cbs?.forEach((cb) => cb());
    });

    expect(result.current).toBe(true);
  });

  it('cleans up event listeners on unmount', () => {
    matchState.set('(max-width: 767px)', false);
    const { unmount } = renderHook(() => useMediaQuery('(max-width: 767px)'));

    const cbs = listeners.get('(max-width: 767px)');
    expect(cbs?.size).toBeGreaterThan(0);

    unmount();

    expect(cbs?.size).toBe(0);
  });

  it('handles different query strings independently', () => {
    matchState.set('(max-width: 767px)', true);
    matchState.set('(min-width: 1025px)', false);

    const { result: mobileResult } = renderHook(() => useMediaQuery('(max-width: 767px)'));
    const { result: desktopResult } = renderHook(() => useMediaQuery('(min-width: 1025px)'));

    expect(mobileResult.current).toBe(true);
    expect(desktopResult.current).toBe(false);
  });
});
