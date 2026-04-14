// Added: DANE API tests for TMAIL-125

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listDanePolicies,
  createDanePolicy,
  deleteDanePolicy,
  lookupTlsa,
  listDaneVerifications,
} from './dane';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('dane API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listDanePolicies', () => {
    it('calls GET /admin/dane', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDanePolicies();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/dane');
      expect(result).toEqual([]);
    });

    it('returns policy objects from API', async () => {
      const mockPolicies = [
        { id: 'p1', domain: 'example.com', enforce: true },
        { id: 'p2', domain: 'test.org', enforce: false },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockPolicies);
      const result = await listDanePolicies();
      expect(result).toHaveLength(2);
      expect(result[0].domain).toBe('example.com');
    });
  });

  describe('createDanePolicy', () => {
    it('calls POST /admin/dane with domain only', async () => {
      const data = { domain: 'example.com' };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'p1', ...data, enforce: false });
      const result = await createDanePolicy(data);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dane', data);
      expect(result.domain).toBe('example.com');
    });

    it('calls POST /admin/dane with enforce flag', async () => {
      const data = { domain: 'secure.org', enforce: true };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'p2', ...data });
      const result = await createDanePolicy(data);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dane', data);
      expect(result.enforce).toBe(true);
    });
  });

  describe('deleteDanePolicy', () => {
    it('calls DELETE /admin/dane/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteDanePolicy('p1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/dane/p1');
    });
  });

  describe('lookupTlsa', () => {
    it('calls POST /admin/dane/lookup with domain', async () => {
      const mockResult = {
        domain: 'example.com',
        status: 'no_tlsa',
        tlsa_records: [],
        message: 'No TLSA records found',
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockResult);
      const result = await lookupTlsa({ domain: 'example.com' });
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dane/lookup', { domain: 'example.com' });
      expect(result.status).toBe('no_tlsa');
    });

    it('calls POST /admin/dane/lookup with port', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        domain: 'mail.example.com',
        status: 'verified',
        tlsa_records: [{ usage: 3, selector: 1, matching_type: 1, cert_data: 'abcdef' }],
        message: 'Found',
      });
      const result = await lookupTlsa({ domain: 'mail.example.com', port: 587 });
      expect(apiClient.post).toHaveBeenCalledWith('/admin/dane/lookup', {
        domain: 'mail.example.com',
        port: 587,
      });
      expect(result.tlsa_records).toHaveLength(1);
    });
  });

  describe('listDaneVerifications', () => {
    it('calls GET /dane/verifications without params', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDaneVerifications();
      expect(apiClient.get).toHaveBeenCalledWith('/dane/verifications');
      expect(result).toEqual([]);
    });

    it('calls GET /dane/verifications with limit and offset', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listDaneVerifications(25, 50);
      expect(apiClient.get).toHaveBeenCalledWith('/dane/verifications?limit=25&offset=50');
    });

    it('returns verification objects', async () => {
      const mockVerifications = [
        {
          id: 'v1',
          user_id: 'u1',
          message_id: '<msg@ex>',
          recipient_domain: 'example.com',
          dane_status: 'verified',
          checked_at: '2026-04-14T10:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockVerifications);
      const result = await listDaneVerifications();
      expect(result[0].dane_status).toBe('verified');
    });
  });
});
