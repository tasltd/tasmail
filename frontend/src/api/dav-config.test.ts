// Added: CalDAV/CardDAV configuration API tests for TMAIL-117

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listDavConfigs,
  createDavConfig,
  getDavConfig,
  updateDavConfig,
  deleteDavConfig,
  syncDavConfig,
  testDavConfig,
} from './dav-config';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('dav-config API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listDavConfigs', () => {
    it('calls GET /dav/configs', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDavConfigs();
      expect(apiClient.get).toHaveBeenCalledWith('/dav/configs');
      expect(result).toEqual([]);
    });
  });

  describe('createDavConfig', () => {
    it('calls POST /dav/configs with full data', async () => {
      const configData = {
        name: 'Radicale',
        server_url: 'https://radicale.example.com',
        username: 'user@example.com',
        password: 'dav-password',
        dav_type: 'both' as const,
        sync_interval_minutes: 30,
        enabled: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dav-1',
        name: 'Radicale',
        server_url: 'https://radicale.example.com',
        dav_type: 'both',
        enabled: true,
      });

      const result = await createDavConfig(configData);
      expect(apiClient.post).toHaveBeenCalledWith('/dav/configs', configData);
      expect(result.name).toBe('Radicale');
    });
  });

  describe('getDavConfig', () => {
    it('calls GET /dav/configs/:id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        id: 'dav-1',
        name: 'Radicale',
        server_url: 'https://radicale.example.com',
      });

      const result = await getDavConfig('dav-1');
      expect(apiClient.get).toHaveBeenCalledWith('/dav/configs/dav-1');
      expect(result.name).toBe('Radicale');
    });
  });

  describe('updateDavConfig', () => {
    it('calls PUT /dav/configs/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'dav-1', server_url: 'https://new-dav.com' });

      await updateDavConfig('dav-1', { server_url: 'https://new-dav.com', sync_interval_minutes: 120 });
      expect(apiClient.put).toHaveBeenCalledWith('/dav/configs/dav-1', {
        server_url: 'https://new-dav.com',
        sync_interval_minutes: 120,
      });
    });
  });

  describe('deleteDavConfig', () => {
    it('calls DELETE /dav/configs/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteDavConfig('dav-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/dav/configs/dav-1');
    });
  });

  describe('syncDavConfig', () => {
    it('calls POST /dav/configs/:id/sync', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'dav-1',
        sync_status: 'syncing',
      });
      const result = await syncDavConfig('dav-1');
      expect(apiClient.post).toHaveBeenCalledWith('/dav/configs/dav-1/sync', {});
      expect(result.sync_status).toBe('syncing');
    });
  });

  describe('testDavConfig', () => {
    it('calls POST /dav/configs/:id/test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: true,
        message: 'Connection successful (HTTP 200)',
        latency_ms: 150,
      });
      const result = await testDavConfig('dav-1');
      expect(apiClient.post).toHaveBeenCalledWith('/dav/configs/dav-1/test', {});
      expect(result.success).toBe(true);
      expect(result.latency_ms).toBe(150);
    });

    it('handles test failure', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: false,
        message: 'Authentication failed',
        latency_ms: 500,
      });
      const result = await testDavConfig('dav-2');
      expect(result.success).toBe(false);
      expect(result.message).toBe('Authentication failed');
    });
  });
});
