// Added: NLP search API tests for TMAIL-135

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { nlpSearch, listNlpHistory, clearNlpHistory } from './nlp-search';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('nlp-search API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('nlpSearch', () => {
    it('calls POST /search/nlp with query', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        query: 'emails from John about budget',
        parsed_params: {
          from: 'John',
          subject: 'budget',
          keywords: ['budget'],
        },
        result_count: 0,
        results: [],
      });

      const result = await nlpSearch('emails from John about budget');
      expect(apiClient.post).toHaveBeenCalledWith('/search/nlp', {
        query: 'emails from John about budget',
      });
      expect(result.parsed_params.from).toBe('John');
      expect(result.parsed_params.subject).toBe('budget');
    });

    it('returns parsed_params and results', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        query: 'attachments from last week',
        parsed_params: {
          has_attachment: true,
          date_from: '2026-04-07',
          date_to: '2026-04-14',
          keywords: [],
        },
        result_count: 3,
        results: [
          { folder: 'INBOX', uid: 1, subject: 'Report', from: 'alice@example.com', date: '2026-04-10' },
          { folder: 'INBOX', uid: 2, subject: 'Invoice', from: 'bob@example.com', date: '2026-04-09' },
          { folder: 'INBOX', uid: 3, subject: 'Photos', from: 'carol@example.com', date: '2026-04-08' },
        ],
      });

      const result = await nlpSearch('attachments from last week');
      expect(result.result_count).toBe(3);
      expect(result.results).toHaveLength(3);
      expect(result.parsed_params.has_attachment).toBe(true);
    });
  });

  describe('listNlpHistory', () => {
    it('calls GET /search/nlp/history', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([
        {
          id: 'hist-1',
          user_id: 'user-1',
          query_text: 'emails about budget',
          parsed_params: { subject: 'budget', keywords: [] },
          result_count: 5,
          created_at: '2026-04-14T10:00:00Z',
        },
      ]);

      const history = await listNlpHistory();
      expect(apiClient.get).toHaveBeenCalledWith('/search/nlp/history');
      expect(history).toHaveLength(1);
      expect(history[0].query_text).toBe('emails about budget');
    });

    it('returns empty array when no history', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);

      const history = await listNlpHistory();
      expect(history).toHaveLength(0);
    });
  });

  describe('clearNlpHistory', () => {
    it('calls DELETE /search/nlp/history', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue({
        deleted: 10,
        message: 'Search history cleared',
      });

      const result = await clearNlpHistory();
      expect(apiClient.delete).toHaveBeenCalledWith('/search/nlp/history');
      expect(result.deleted).toBe(10);
      expect(result.message).toBe('Search history cleared');
    });
  });
});
