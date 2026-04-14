// Added: Archive API tests for TMAIL-107

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listArchivePolicies,
  createArchivePolicy,
  updateArchivePolicy,
  deleteArchivePolicy,
  getArchiveConfig,
  updateArchiveConfig,
  searchArchive,
  getArchiveSearchHistory,
} from './archive';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('archive API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listArchivePolicies', () => {
    it('calls GET /admin/archive/policies', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listArchivePolicies();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/archive/policies');
      expect(result).toEqual([]);
    });
  });

  describe('createArchivePolicy', () => {
    it('calls POST /admin/archive/policies with policy data', async () => {
      const policyData = {
        name: 'Archive INBOX',
        match_criteria: { folders: ['INBOX'] },
        archive_after_days: 90,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'pol-1', ...policyData });

      const result = await createArchivePolicy(policyData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/archive/policies', policyData);
      expect(result.name).toBe('Archive INBOX');
    });
  });

  describe('updateArchivePolicy', () => {
    it('calls PUT /admin/archive/policies/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'pol-1', enabled: false });

      await updateArchivePolicy('pol-1', { enabled: false });
      expect(apiClient.put).toHaveBeenCalledWith('/admin/archive/policies/pol-1', { enabled: false });
    });
  });

  describe('deleteArchivePolicy', () => {
    it('calls DELETE /admin/archive/policies/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteArchivePolicy('pol-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/archive/policies/pol-1');
    });
  });

  describe('getArchiveConfig', () => {
    it('calls GET /admin/archive/config', async () => {
      const mockConfig = {
        id: 'cfg-1',
        piler_url: 'https://piler.example.com',
        retention_years: 7,
        enabled: true,
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockConfig);
      const result = await getArchiveConfig();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/archive/config');
      expect(result).toEqual(mockConfig);
    });

    it('returns null when no config exists', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(null);
      const result = await getArchiveConfig();
      expect(result).toBeNull();
    });
  });

  describe('updateArchiveConfig', () => {
    it('calls PUT /admin/archive/config with config data', async () => {
      const configData = { piler_url: 'https://piler.local', enabled: true };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'cfg-1', ...configData });

      const result = await updateArchiveConfig(configData);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/archive/config', configData);
      expect(result.piler_url).toBe('https://piler.local');
    });
  });

  describe('searchArchive', () => {
    it('calls POST /archive/search with search request', async () => {
      const searchData = { query: 'invoice', date_from: '2025-01-01' };
      vi.mocked(apiClient.post).mockResolvedValue([]);

      const result = await searchArchive(searchData);
      expect(apiClient.post).toHaveBeenCalledWith('/archive/search', searchData);
      expect(result).toEqual([]);
    });

    it('returns search results from Piler', async () => {
      const searchData = { query: 'report' };
      const mockResults = [
        {
          id: 'piler-1',
          subject: 'Q4 Report',
          sender: 'cfo@example.com',
          recipients: ['board@example.com'],
          date: '2025-12-15T10:00:00Z',
          size: 102400,
          has_attachment: true,
        },
      ];
      vi.mocked(apiClient.post).mockResolvedValue(mockResults);

      const result = await searchArchive(searchData);
      expect(result).toHaveLength(1);
      expect(result[0].subject).toBe('Q4 Report');
    });
  });

  describe('getArchiveSearchHistory', () => {
    it('calls GET /archive/search/history', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await getArchiveSearchHistory();
      expect(apiClient.get).toHaveBeenCalledWith('/archive/search/history');
      expect(result).toEqual([]);
    });

    it('returns history entries with timestamps', async () => {
      const mockHistory = [
        {
          id: 'search-1',
          user_id: 'user-1',
          query: 'invoice',
          filters: { date_from: '2025-01-01' },
          result_count: 5,
          searched_at: '2026-04-14T10:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockHistory);

      const result = await getArchiveSearchHistory();
      expect(result).toHaveLength(1);
      expect(result[0].query).toBe('invoice');
      expect(result[0].result_count).toBe(5);
    });
  });
});
