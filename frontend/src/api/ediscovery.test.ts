// Added: eDiscovery API client tests for TMAIL-137

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listEdiscoverySearches,
  createEdiscoverySearch,
  getEdiscoverySearch,
  deleteEdiscoverySearch,
  executeEdiscoverySearch,
  exportEdiscoveryResults,
} from './ediscovery';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('ediscovery API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listEdiscoverySearches', () => {
    it('calls GET /admin/ediscovery', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listEdiscoverySearches();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ediscovery');
      expect(result).toEqual([]);
    });
  });

  describe('createEdiscoverySearch', () => {
    it('calls POST /admin/ediscovery with search data', async () => {
      const searchData = {
        name: 'Q1 Investigation',
        description: 'Looking for contract data',
        search_query: 'contract breach',
        target_users: ['user-123'],
        date_from: '2026-01-01T00:00:00Z',
        date_to: '2026-04-01T00:00:00Z',
        include_attachments: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'ed-1', ...searchData, status: 'Pending' });

      const result = await createEdiscoverySearch(searchData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ediscovery', searchData);
      expect(result.name).toBe('Q1 Investigation');
      expect(result.status).toBe('Pending');
    });
  });

  describe('getEdiscoverySearch', () => {
    it('calls GET /admin/ediscovery/:id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        id: 'ed-1',
        name: 'Test Search',
        results: [],
      });

      const result = await getEdiscoverySearch('ed-1');
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ediscovery/ed-1');
      expect(result.name).toBe('Test Search');
      expect(result.results).toEqual([]);
    });
  });

  describe('deleteEdiscoverySearch', () => {
    it('calls DELETE /admin/ediscovery/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteEdiscoverySearch('ed-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/ediscovery/ed-1');
    });
  });

  describe('executeEdiscoverySearch', () => {
    it('calls POST /admin/ediscovery/:id/execute', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'ed-1', status: 'Running' });

      const result = await executeEdiscoverySearch('ed-1');
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ediscovery/ed-1/execute', {});
      expect(result.status).toBe('Running');
    });
  });

  describe('exportEdiscoveryResults', () => {
    it('calls POST /admin/ediscovery/:id/export', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'ed-1',
        status: 'Exported',
        export_path: '/exports/ediscovery/ed-1.mbox',
      });

      const result = await exportEdiscoveryResults('ed-1');
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ediscovery/ed-1/export', {});
      expect(result.status).toBe('Exported');
      expect(result.export_path).toBe('/exports/ediscovery/ed-1.mbox');
    });
  });
});
