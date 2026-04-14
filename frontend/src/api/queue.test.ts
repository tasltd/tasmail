// Added: Unit tests for email queue API client (TMAIL-58)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchQueueItems, fetchQueueStats, cancelQueueItem, retryQueueItem } from './queue';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('queue API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchQueueItems', () => {
    it('calls GET /queue without status filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await fetchQueueItems();
      expect(apiClient.get).toHaveBeenCalledWith('/queue');
      expect(result).toEqual([]);
    });

    it('calls GET /queue?status=pending when filtered', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchQueueItems('pending');
      expect(apiClient.get).toHaveBeenCalledWith('/queue?status=pending');
    });

    it('calls GET /queue?status=failed when filtered by failed', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchQueueItems('failed');
      expect(apiClient.get).toHaveBeenCalledWith('/queue?status=failed');
    });
  });

  describe('fetchQueueStats', () => {
    it('calls GET /queue/stats and returns stats', async () => {
      const mockStats = { pending: 5, sending: 1, sent: 100, failed: 2, dead_letter: 0 };
      vi.mocked(apiClient.get).mockResolvedValue(mockStats);
      const result = await fetchQueueStats();
      expect(apiClient.get).toHaveBeenCalledWith('/queue/stats');
      expect(result).toEqual(mockStats);
    });
  });

  describe('cancelQueueItem', () => {
    it('calls DELETE /queue/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await cancelQueueItem('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/queue/abc-123');
    });
  });

  describe('retryQueueItem', () => {
    it('calls POST /queue/:id/retry', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await retryQueueItem('abc-123');
      expect(apiClient.post).toHaveBeenCalledWith('/queue/abc-123/retry');
    });

    it('handles retry of dead_letter item', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await retryQueueItem('dead-item-456');
      expect(apiClient.post).toHaveBeenCalledWith('/queue/dead-item-456/retry');
    });
  });
});
