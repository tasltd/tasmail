// Added: SMTP configuration API tests for TMAIL-48

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listSmtpConfigs,
  createSmtpConfig,
  getSmtpConfig,
  updateSmtpConfig,
  deleteSmtpConfig,
  testSmtpConfig,
  setDefaultSmtp,
} from './smtp-config';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('smtp-config API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listSmtpConfigs', () => {
    it('calls GET /smtp-configs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listSmtpConfigs();
      expect(apiClient.get).toHaveBeenCalledWith('/smtp-configs');
      expect(result).toEqual([]);
    });
  });

  describe('createSmtpConfig', () => {
    it('calls POST /smtp-configs with full data', async () => {
      const configData = {
        name: 'Gmail SMTP',
        host: 'smtp.gmail.com',
        port: 587,
        username: 'user@gmail.com',
        password: 'app-password',
        encryption: 'starttls' as const,
        from_address: 'user@gmail.com',
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'smtp-1',
        name: 'Gmail SMTP',
        host: 'smtp.gmail.com',
        port: 587,
        is_default: false,
      });

      const result = await createSmtpConfig(configData);
      expect(apiClient.post).toHaveBeenCalledWith('/smtp-configs', configData);
      expect(result.name).toBe('Gmail SMTP');
    });
  });

  describe('getSmtpConfig', () => {
    it('calls GET /smtp-configs/:id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        id: 'smtp-1',
        name: 'Gmail',
        host: 'smtp.gmail.com',
      });

      const result = await getSmtpConfig('smtp-1');
      expect(apiClient.get).toHaveBeenCalledWith('/smtp-configs/smtp-1');
      expect(result.name).toBe('Gmail');
    });
  });

  describe('updateSmtpConfig', () => {
    it('calls PUT /smtp-configs/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'smtp-1', host: 'new-host.com' });

      await updateSmtpConfig('smtp-1', { host: 'new-host.com', port: 465 });
      expect(apiClient.put).toHaveBeenCalledWith('/smtp-configs/smtp-1', {
        host: 'new-host.com',
        port: 465,
      });
    });
  });

  describe('deleteSmtpConfig', () => {
    it('calls DELETE /smtp-configs/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteSmtpConfig('smtp-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/smtp-configs/smtp-1');
    });
  });

  describe('testSmtpConfig', () => {
    it('calls POST /smtp-configs/:id/test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: true,
        message: 'SMTP connection successful. Test email sent to user@gmail.com',
        latency_ms: 234,
      });
      const result = await testSmtpConfig('smtp-1');
      expect(apiClient.post).toHaveBeenCalledWith('/smtp-configs/smtp-1/test', {});
      expect(result.success).toBe(true);
      expect(result.latency_ms).toBe(234);
    });

    it('handles test failure', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: false,
        message: 'Authentication failed',
        latency_ms: 500,
      });
      const result = await testSmtpConfig('smtp-2');
      expect(result.success).toBe(false);
      expect(result.message).toBe('Authentication failed');
    });
  });

  describe('setDefaultSmtp', () => {
    it('calls POST /smtp-configs/:id/default', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'smtp-1',
        name: 'Gmail',
        is_default: true,
      });
      const result = await setDefaultSmtp('smtp-1');
      expect(apiClient.post).toHaveBeenCalledWith('/smtp-configs/smtp-1/default', {});
      expect(result.is_default).toBe(true);
    });
  });
});
