// Added: Custom hostname API tests for TMAIL-112

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listHostnames,
  createHostname,
  getHostname,
  updateHostname,
  deleteHostname,
  verifyHostname,
} from './custom-hostnames';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('custom-hostnames API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listHostnames', () => {
    it('calls GET /admin/hostnames', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listHostnames();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/hostnames');
      expect(result).toEqual([]);
    });
  });

  describe('createHostname', () => {
    it('calls POST /admin/hostnames with full data', async () => {
      const hostnameData = {
        domain_id: 'domain-1',
        smtp_hostname: 'smtp.acme.com',
        imap_hostname: 'imap.acme.com',
        webmail_hostname: 'mail.acme.com',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'h-1', ...hostnameData });

      const result = await createHostname(hostnameData);
      expect(apiClient.post).toHaveBeenCalledWith('/admin/hostnames', hostnameData);
      expect(result.smtp_hostname).toBe('smtp.acme.com');
    });
  });

  describe('getHostname', () => {
    it('calls GET /admin/hostnames/:id', async () => {
      const mockHostname = { id: 'h-1', smtp_hostname: 'smtp.acme.com' };
      vi.mocked(apiClient.get).mockResolvedValue(mockHostname);

      const result = await getHostname('h-1');
      expect(apiClient.get).toHaveBeenCalledWith('/admin/hostnames/h-1');
      expect(result.id).toBe('h-1');
    });
  });

  describe('updateHostname', () => {
    it('calls PUT /admin/hostnames/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'h-1', smtp_hostname: 'new-smtp.acme.com' });

      await updateHostname('h-1', { smtp_hostname: 'new-smtp.acme.com' });
      expect(apiClient.put).toHaveBeenCalledWith('/admin/hostnames/h-1', { smtp_hostname: 'new-smtp.acme.com' });
    });
  });

  describe('deleteHostname', () => {
    it('calls DELETE /admin/hostnames/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteHostname('h-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/hostnames/h-1');
    });
  });

  describe('verifyHostname', () => {
    it('calls POST /admin/hostnames/:id/verify', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'h-1', verified: true });
      const result = await verifyHostname('h-1');
      expect(apiClient.post).toHaveBeenCalledWith('/admin/hostnames/h-1/verify', {});
      expect(result.verified).toBe(true);
    });
  });
});
