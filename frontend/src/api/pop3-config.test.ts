// Added: POP3 configuration API tests for TMAIL-133

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  getPop3Config,
  updatePop3Config,
  deletePop3Config,
  getPop3Status,
} from './pop3-config';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('pop3-config API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getPop3Config', () => {
    it('calls GET /pop3/config', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        id: 'pop3-1',
        user_id: 'user-1',
        enabled: true,
        delete_after_download: false,
        retention_days: null,
      });
      const result = await getPop3Config();
      expect(apiClient.get).toHaveBeenCalledWith('/pop3/config');
      expect(result?.enabled).toBe(true);
    });

    it('returns null when no config exists', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(null);
      const result = await getPop3Config();
      expect(apiClient.get).toHaveBeenCalledWith('/pop3/config');
      expect(result).toBeNull();
    });
  });

  describe('updatePop3Config', () => {
    it('calls PUT /pop3/config with full data', async () => {
      const configData = {
        enabled: true,
        delete_after_download: true,
        retention_days: 30,
      };
      vi.mocked(apiClient.put).mockResolvedValue({
        id: 'pop3-1',
        enabled: true,
        delete_after_download: true,
        retention_days: 30,
      });

      const result = await updatePop3Config(configData);
      expect(apiClient.put).toHaveBeenCalledWith('/pop3/config', configData);
      expect(result.enabled).toBe(true);
      expect(result.retention_days).toBe(30);
    });

    it('calls PUT /pop3/config with partial data', async () => {
      const configData = { enabled: false };
      vi.mocked(apiClient.put).mockResolvedValue({
        id: 'pop3-1',
        enabled: false,
        delete_after_download: false,
        retention_days: null,
      });

      const result = await updatePop3Config(configData);
      expect(apiClient.put).toHaveBeenCalledWith('/pop3/config', configData);
      expect(result.enabled).toBe(false);
    });

    it('handles setting retention_days to null', async () => {
      const configData = { retention_days: null };
      vi.mocked(apiClient.put).mockResolvedValue({
        id: 'pop3-1',
        enabled: true,
        delete_after_download: false,
        retention_days: null,
      });

      await updatePop3Config(configData);
      expect(apiClient.put).toHaveBeenCalledWith('/pop3/config', { retention_days: null });
    });
  });

  describe('deletePop3Config', () => {
    it('calls DELETE /pop3/config', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deletePop3Config();
      expect(apiClient.delete).toHaveBeenCalledWith('/pop3/config');
    });
  });

  describe('getPop3Status', () => {
    it('calls GET /pop3/status and returns connection info', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        server: 'mail.example.com',
        port: 995,
        encryption: 'SSL/TLS',
        username_format: 'user@mail.example.com',
      });

      const result = await getPop3Status();
      expect(apiClient.get).toHaveBeenCalledWith('/pop3/status');
      expect(result.server).toBe('mail.example.com');
      expect(result.port).toBe(995);
      expect(result.encryption).toBe('SSL/TLS');
    });
  });
});
