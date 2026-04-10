import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import React from 'react';

const mockFetchFolders = vi.fn();
const mockFetchMessages = vi.fn();
const mockFetchMessage = vi.fn();
const mockSearchMessages = vi.fn();

vi.mock('../api/folders', () => ({
  fetchFolders: (...args: unknown[]) => mockFetchFolders(...args),
}));

vi.mock('../api/messages', () => ({
  fetchMessages: (...args: unknown[]) => mockFetchMessages(...args),
  fetchMessage: (...args: unknown[]) => mockFetchMessage(...args),
  searchMessages: (...args: unknown[]) => mockSearchMessages(...args),
}));

vi.mock('../utils/offline-cache', () => ({
  offlineCache: {
    cacheFolders: vi.fn(),
    getFolders: vi.fn(),
    cacheMessages: vi.fn(),
    getMessages: vi.fn(),
    cacheFullMessage: vi.fn(),
    getFullMessage: vi.fn(),
  },
}));

vi.mock('../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ selectedFolder: 'INBOX', selectedUid: 1 }),
}));

import { useFolders, useMessages, useSearch } from './useMailbox';

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return React.createElement(QueryClientProvider, { client: queryClient }, children);
  };
}

describe('useMailbox hooks', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('useFolders', () => {
    it('calls fetchFolders and returns data', async () => {
      const folders = [{ name: 'INBOX', delimiter: '/', children: [] }];
      mockFetchFolders.mockResolvedValue(folders);

      const { result } = renderHook(() => useFolders(), { wrapper: createWrapper() });

      await waitFor(() => expect(result.current.isSuccess).toBe(true));
      expect(result.current.data).toEqual(folders);
      expect(mockFetchFolders).toHaveBeenCalled();
    });
  });

  describe('useMessages', () => {
    it('is disabled when folder is empty string', () => {
      const { result } = renderHook(() => useMessages('', 0, 50), {
        wrapper: createWrapper(),
      });

      // enabled: !!folder is false for empty string, so query should not fetch
      expect(result.current.fetchStatus).toBe('idle');
      expect(mockFetchMessages).not.toHaveBeenCalled();
    });
  });

  describe('useSearch', () => {
    it('is disabled when query length < 2', () => {
      const { result } = renderHook(() => useSearch('a'), {
        wrapper: createWrapper(),
      });

      expect(result.current.fetchStatus).toBe('idle');
      expect(mockSearchMessages).not.toHaveBeenCalled();
    });
  });
});
