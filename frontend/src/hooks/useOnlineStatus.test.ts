import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

vi.mock('../utils/background-sync', () => ({
  backgroundSync: { processPending: vi.fn().mockResolvedValue({ processed: 0, failed: 0 }) },
}));

import { useOnlineStatus, PERIODIC_SYNC_MS } from './useOnlineStatus';
import { backgroundSync } from '../utils/background-sync';

describe('useOnlineStatus', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns true when navigator.onLine is true', () => {
    const { result } = renderHook(() => useOnlineStatus());
    // jsdom defaults navigator.onLine to true
    expect(result.current).toBe(true);
  });

  it('subscribes to online/offline events', () => {
    const addSpy = vi.spyOn(window, 'addEventListener');
    const removeSpy = vi.spyOn(window, 'removeEventListener');

    const { unmount } = renderHook(() => useOnlineStatus());

    expect(addSpy).toHaveBeenCalledWith('online', expect.any(Function));
    expect(addSpy).toHaveBeenCalledWith('offline', expect.any(Function));

    unmount();

    expect(removeSpy).toHaveBeenCalledWith('online', expect.any(Function));
    expect(removeSpy).toHaveBeenCalledWith('offline', expect.any(Function));

    addSpy.mockRestore();
    removeSpy.mockRestore();
  });

  describe('periodic sync fallback (TMAIL-88)', () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it('calls processPending on a PERIODIC_SYNC_MS interval while online', async () => {
      vi.useFakeTimers();
      Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
      const { unmount } = renderHook(() => useOnlineStatus());

      // Initial replay fires once on mount (online effect)
      await act(async () => { await Promise.resolve(); });
      const initialCalls = (backgroundSync.processPending as ReturnType<typeof vi.fn>).mock.calls.length;
      expect(initialCalls).toBeGreaterThanOrEqual(1);

      await act(async () => { vi.advanceTimersByTime(PERIODIC_SYNC_MS); });
      expect((backgroundSync.processPending as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThan(initialCalls);

      const afterFirstTick = (backgroundSync.processPending as ReturnType<typeof vi.fn>).mock.calls.length;
      await act(async () => { vi.advanceTimersByTime(PERIODIC_SYNC_MS); });
      expect((backgroundSync.processPending as ReturnType<typeof vi.fn>).mock.calls.length).toBeGreaterThan(afterFirstTick);

      unmount();
    });

    it('does NOT run the periodic interval while offline', async () => {
      vi.useFakeTimers();
      Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
      const { unmount } = renderHook(() => useOnlineStatus());

      (backgroundSync.processPending as ReturnType<typeof vi.fn>).mockClear();
      await act(async () => { vi.advanceTimersByTime(PERIODIC_SYNC_MS * 3); });
      expect(backgroundSync.processPending).not.toHaveBeenCalled();

      unmount();
      Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
    });
  });
});
