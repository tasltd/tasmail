import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';

vi.mock('../utils/background-sync', () => ({
  backgroundSync: { processPending: vi.fn().mockResolvedValue({ processed: 0, failed: 0 }) },
}));

import { useOnlineStatus } from './useOnlineStatus';

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
});
