// Added: Deliverability API tests for TMAIL-39

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  runDeliverabilityCheck,
  getExternalDeliverabilityTools,
} from './deliverability';
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

  describe('getExternalDeliverabilityTools', () => {
    // Added: TMAIL-39 — guard the external tools fetch shape end-to-end.
    const sampleResponse = {
      mail_tester: {
        test_address: 'test-tasmail-abc123@mail-tester.com',
        report_url: 'https://www.mail-tester.com/test-tasmail-abc123',
        expires_in_minutes: 45,
        instructions: 'Send an email to the address.',
      },
      google_postmaster: {
        dashboard_url:
          'https://postmaster.google.com/managedomains?domain=mail.example.com',
        instructions: 'Sign in and verify.',
      },
      providers: [
        { name: 'Gmail', spam_folder_label: 'Spam', instructions: '...' },
        { name: 'Outlook / Hotmail', spam_folder_label: 'Junk', instructions: '...' },
        { name: 'Yahoo Mail', spam_folder_label: 'Bulk', instructions: '...' },
        { name: 'ProtonMail', spam_folder_label: 'Spam', instructions: '...' },
      ],
    };

    it('calls GET /admin/deliverability/external-tools with domain query', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(sampleResponse);
      const result = await getExternalDeliverabilityTools('mail.example.com');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/admin/deliverability/external-tools?domain=mail.example.com',
      );
      expect(result.mail_tester.test_address).toBe(
        'test-tasmail-abc123@mail-tester.com',
      );
      expect(result.google_postmaster.dashboard_url).toContain('mail.example.com');
      expect(result.providers).toHaveLength(4);
    });

    it('omits the domain query when domain is blank', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(sampleResponse);
      await getExternalDeliverabilityTools('   ');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/admin/deliverability/external-tools',
      );
    });

    it('URL-encodes punctuation in the domain', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(sampleResponse);
      await getExternalDeliverabilityTools('a b.com');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/admin/deliverability/external-tools?domain=a%20b.com',
      );
    });

    it('exposes the four provider entries called out in the TMAIL-39 spec', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(sampleResponse);
      const result = await getExternalDeliverabilityTools('mail.example.com');
      const names = result.providers.map((p) => p.name);
      expect(names).toEqual(
        expect.arrayContaining([
          expect.stringContaining('Gmail'),
          expect.stringContaining('Outlook'),
          expect.stringContaining('Yahoo'),
          expect.stringContaining('ProtonMail'),
        ]),
      );
    });

    it('propagates API errors', async () => {
      vi.mocked(apiClient.get).mockRejectedValue(new Error('boom'));
      await expect(
        getExternalDeliverabilityTools('mail.example.com'),
      ).rejects.toThrow('boom');
    });
  });
});
