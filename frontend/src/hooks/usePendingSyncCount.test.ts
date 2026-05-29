import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import 'fake-indexeddb/auto';

vi.mock('../api/scheduled', () => ({ scheduledApi: { scheduleSend: vi.fn() } }));
vi.mock('../api/messages', () => ({
  moveMessage: vi.fn(),
  deleteMessage: vi.fn(),
  flagMessage: vi.fn(),
  saveDraft: vi.fn(),
}));

import { backgroundSync } from '../utils/background-sync';
import { usePendingSyncCount } from './usePendingSyncCount';

describe('usePendingSyncCount', () => {
  beforeEach(async () => {
    await backgroundSync.clearAll();
  });
  afterEach(() => vi.restoreAllMocks());

  it('returns 0 when queue is empty', async () => {
    const { result } = renderHook(() => usePendingSyncCount());
    await waitFor(() => expect(result.current).toBe(0));
  });

  it('reflects current queue size after enqueue', async () => {
    const { result } = renderHook(() => usePendingSyncCount());
    await waitFor(() => expect(result.current).toBe(0));

    await act(async () => {
      await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'A' });
    });
    await waitFor(() => expect(result.current).toBe(1));

    await act(async () => {
      await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 2 });
    });
    await waitFor(() => expect(result.current).toBe(2));
  });

  it('drops to 0 after clearAll', async () => {
    await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'A' });
    const { result } = renderHook(() => usePendingSyncCount());
    await waitFor(() => expect(result.current).toBe(1));

    await act(async () => {
      await backgroundSync.clearAll();
    });
    await waitFor(() => expect(result.current).toBe(0));
  });
});
