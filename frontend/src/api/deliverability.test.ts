// Added: Deliverability API tests for TMAIL-39

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { runDeliverabilityCheck } from './deliverability';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('deliverability API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('runDeliverabilityCheck', () => {
    it('calls GET /admin/deliverability/check with domain param', async () => {
      const mockReport = {
        domain: 'mail.example.com',
        checks: [
          { name: 'SPF Record', status: 'pass', details: 'v=spf1 found' },
          { name: 'DKIM Record', status: 'fail', details: 'No DKIM record' },
        ],
        score: 50,
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockReport);
      const result = await runDeliverabilityCheck('mail.example.com');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/admin/deliverability/check?domain=mail.example.com',
      );
      expect(result.domain).toBe('mail.example.com');
      expect(result.score).toBe(50);
      expect(result.checks).toHaveLength(2);
    });

    it('encodes special characters in domain', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        domain: 'test domain',
        checks: [],
        score: 0,
      });
      await runDeliverabilityCheck('test domain');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/admin/deliverability/check?domain=test%20domain',
      );
    });

    it('returns all check statuses correctly', async () => {
      const mockReport = {
        domain: 'example.com',
        checks: [
          { name: 'SPF', status: 'pass', details: 'ok' },
          { name: 'DKIM', status: 'fail', details: 'missing' },
          { name: 'DMARC', status: 'warn', details: 'p=none' },
          { name: 'TLS', status: 'error', details: 'timeout' },
        ],
        score: 38,
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockReport);
      const result = await runDeliverabilityCheck('example.com');
      expect(result.checks[0].status).toBe('pass');
      expect(result.checks[1].status).toBe('fail');
      expect(result.checks[2].status).toBe('warn');
      expect(result.checks[3].status).toBe('error');
    });

    it('returns empty checks for new domain', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        domain: 'new.example.com',
        checks: [],
        score: 0,
      });
      const result = await runDeliverabilityCheck('new.example.com');
      expect(result.checks).toEqual([]);
      expect(result.score).toBe(0);
    });

    it('propagates API errors', async () => {
      vi.mocked(apiClient.get).mockRejectedValue(new Error('Network error'));
      await expect(runDeliverabilityCheck('example.com')).rejects.toThrow('Network error');
    });
  });
});
