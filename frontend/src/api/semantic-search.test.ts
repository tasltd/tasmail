// Added: Semantic search API tests for TMAIL-106

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { semanticSearch, indexEmail, getIndexStats } from './semantic-search';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('semantic-search API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('semanticSearch', () => {
    it('calls POST /search/semantic with query', async () => {
      vi.mocked(apiClient.post).mockResolvedValue([
        {
          folder: 'INBOX',
          uid: 42,
          subject: 'Quarterly Report',
          similarity_score: 0.89,
        },
      ]);

      const results = await semanticSearch('quarterly budget review');
      expect(apiClient.post).toHaveBeenCalledWith('/search/semantic', {
        query: 'quarterly budget review',
      });
      expect(results).toHaveLength(1);
      expect(results[0].similarity_score).toBe(0.89);
    });

    it('passes optional limit parameter', async () => {
      vi.mocked(apiClient.post).mockResolvedValue([]);

      await semanticSearch('meeting notes', 5);
      expect(apiClient.post).toHaveBeenCalledWith('/search/semantic', {
        query: 'meeting notes',
        limit: 5,
      });
    });
  });

  describe('indexEmail', () => {
    it('calls POST /search/index with email data', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'emb-1',
        folder: 'INBOX',
        uid: 42,
        model_used: 'text-embedding-3-small',
        indexed: true,
      });

      const result = await indexEmail('INBOX', 42, 'Email body text', 'Subject line');
      expect(apiClient.post).toHaveBeenCalledWith('/search/index', {
        folder: 'INBOX',
        uid: 42,
        text: 'Email body text',
        subject: 'Subject line',
      });
      expect(result.indexed).toBe(true);
    });
  });

  describe('getIndexStats', () => {
    it('calls GET /search/index/stats', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        total_indexed: 150,
        per_folder: [
          { folder: 'INBOX', count: 100 },
          { folder: 'Sent', count: 50 },
        ],
      });

      const stats = await getIndexStats();
      expect(apiClient.get).toHaveBeenCalledWith('/search/index/stats');
      expect(stats.total_indexed).toBe(150);
      expect(stats.per_folder).toHaveLength(2);
    });
  });
});
