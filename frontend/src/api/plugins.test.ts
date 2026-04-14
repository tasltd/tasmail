// Added: Plugin API tests for TMAIL-132

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listPlugins,
  createPlugin,
  getPlugin,
  updatePlugin,
  deletePlugin,
  listExecutions,
  testPlugin,
} from './plugins';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('plugins API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listPlugins', () => {
    it('calls GET /plugins', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listPlugins();
      expect(apiClient.get).toHaveBeenCalledWith('/plugins');
      expect(result).toEqual([]);
    });
  });

  describe('createPlugin', () => {
    it('calls POST /plugins with full data', async () => {
      const pluginData = {
        name: 'Slack Notifier',
        description: 'Posts to Slack on new email',
        plugin_type: 'webhook' as const,
        config: { url: 'https://hooks.slack.com/services/...' },
        hooks: ['on_receive' as const, 'on_send' as const],
        enabled: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'p-1', ...pluginData });

      const result = await createPlugin(pluginData);
      expect(apiClient.post).toHaveBeenCalledWith('/plugins', pluginData);
      expect(result.name).toBe('Slack Notifier');
    });

    it('calls POST /plugins with minimal data', async () => {
      const pluginData = {
        name: 'Basic Filter',
        plugin_type: 'filter' as const,
        hooks: ['on_receive' as const],
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'p-2', ...pluginData });

      const result = await createPlugin(pluginData);
      expect(apiClient.post).toHaveBeenCalledWith('/plugins', pluginData);
      expect(result.id).toBe('p-2');
    });
  });

  describe('getPlugin', () => {
    it('calls GET /plugins/:id', async () => {
      const mockPlugin = { id: 'p-1', name: 'My Plugin' };
      vi.mocked(apiClient.get).mockResolvedValue(mockPlugin);

      const result = await getPlugin('p-1');
      expect(apiClient.get).toHaveBeenCalledWith('/plugins/p-1');
      expect(result.id).toBe('p-1');
    });
  });

  describe('updatePlugin', () => {
    it('calls PUT /plugins/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'p-1', enabled: false });

      await updatePlugin('p-1', { enabled: false });
      expect(apiClient.put).toHaveBeenCalledWith('/plugins/p-1', { enabled: false });
    });

    it('calls PUT /plugins/:id with name update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'p-1', name: 'Renamed' });

      await updatePlugin('p-1', { name: 'Renamed' });
      expect(apiClient.put).toHaveBeenCalledWith('/plugins/p-1', { name: 'Renamed' });
    });
  });

  describe('deletePlugin', () => {
    it('calls DELETE /plugins/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deletePlugin('p-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/plugins/p-1');
    });
  });

  describe('listExecutions', () => {
    it('calls GET /plugins/:id/executions', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listExecutions('p-1');
      expect(apiClient.get).toHaveBeenCalledWith('/plugins/p-1/executions');
      expect(result).toEqual([]);
    });

    it('returns execution records', async () => {
      const executions = [
        {
          id: 'e-1',
          plugin_id: 'p-1',
          event: 'on_receive',
          status: 'success',
          duration_ms: 150,
          error_message: null,
          executed_at: '2026-04-10T12:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(executions);

      const result = await listExecutions('p-1');
      expect(result).toHaveLength(1);
      expect(result[0].status).toBe('success');
    });
  });

  describe('testPlugin', () => {
    it('calls POST /plugins/:id/test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: true,
        duration_ms: 100,
        error: null,
      });

      const result = await testPlugin('p-1');
      expect(apiClient.post).toHaveBeenCalledWith('/plugins/p-1/test', {});
      expect(result.success).toBe(true);
    });

    it('returns error for failed test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: false,
        duration_ms: 5000,
        error: 'Connection refused',
      });

      const result = await testPlugin('p-1');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Connection refused');
    });
  });
});
