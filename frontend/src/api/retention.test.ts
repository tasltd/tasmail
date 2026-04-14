// Added: Retention policy and legal hold API tests for TMAIL-109

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listRetentionPolicies,
  createRetentionPolicy,
  updateRetentionPolicy,
  deleteRetentionPolicy,
  listLegalHolds,
  createLegalHold,
  releaseLegalHold,
} from './retention';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('retention API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listRetentionPolicies', () => {
    it('calls GET /admin/retention', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listRetentionPolicies();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/retention');
      expect(result).toEqual([]);
    });
  });

  describe('createRetentionPolicy', () => {
    it('calls POST /admin/retention with policy data', async () => {
      const policyData = {
        name: 'Trash cleanup',
        description: 'Auto-delete trash after 30 days',
        retention_days: 30,
        folder_pattern: 'Trash',
        apply_to_all: false,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'rp-1', ...policyData });

      const result = await createRetentionPolicy(policyData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/retention', policyData);
      expect(result.name).toBe('Trash cleanup');
    });
  });

  describe('updateRetentionPolicy', () => {
    it('calls PUT /admin/retention/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'rp-1', retention_days: 60 });

      await updateRetentionPolicy('rp-1', { retention_days: 60 });
      expect(apiClient.put).toHaveBeenCalledWith('/admin/retention/rp-1', { retention_days: 60 });
    });
  });

  describe('deleteRetentionPolicy', () => {
    it('calls DELETE /admin/retention/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteRetentionPolicy('rp-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/retention/rp-1');
    });
  });

  describe('listLegalHolds', () => {
    it('calls GET /admin/legal-holds', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listLegalHolds();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/legal-holds');
      expect(result).toEqual([]);
    });
  });

  describe('createLegalHold', () => {
    it('calls POST /admin/legal-holds with hold data', async () => {
      const holdData = {
        user_id: 'user-123',
        reason: 'Ongoing litigation',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'lh-1', ...holdData, active: true });

      const result = await createLegalHold(holdData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/legal-holds', holdData);
      expect(result.active).toBe(true);
    });
  });

  describe('releaseLegalHold', () => {
    it('calls PUT /admin/legal-holds/:id/release', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'lh-1', active: false });

      const result = await releaseLegalHold('lh-1');
      expect(apiClient.put).toHaveBeenCalledWith('/admin/legal-holds/lh-1/release', {});
      expect(result.active).toBe(false);
    });
  });
});
