// Added: Ollama API client tests for TMAIL-102

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  getOllamaConfig,
  updateOllamaConfig,
  getOllamaStatus,
  pullOllamaModel,
  deleteOllamaModel,
  listCachedModels,
  formatModelSize,
} from './ollama';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('ollama API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('getOllamaConfig', () => {
    it('calls GET /admin/ollama/config', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        id: 'cfg-1',
        base_url: 'http://localhost:11434',
        enabled: false,
        default_model: 'llama3.2',
      });
      const result = await getOllamaConfig();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ollama/config');
      expect(result.base_url).toBe('http://localhost:11434');
    });
  });

  describe('updateOllamaConfig', () => {
    it('calls PUT /admin/ollama/config with data', async () => {
      const data = { enabled: true, default_model: 'mistral' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'cfg-1', ...data });
      const result = await updateOllamaConfig(data);
      expect(apiClient.put).toHaveBeenCalledWith('/admin/ollama/config', data);
      expect(result.enabled).toBe(true);
    });
  });

  describe('getOllamaStatus', () => {
    it('calls GET /admin/ollama/status', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({
        running: true,
        version: '0.3.14',
        models: [{ name: 'llama3.2', size: 4100000000 }],
      });
      const result = await getOllamaStatus();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ollama/status');
      expect(result.running).toBe(true);
      expect(result.models).toHaveLength(1);
    });
  });

  describe('pullOllamaModel', () => {
    it('calls POST /admin/ollama/models/pull with model name', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: true,
        message: 'success',
      });
      const result = await pullOllamaModel('codellama');
      expect(apiClient.post).toHaveBeenCalledWith('/admin/ollama/models/pull', {
        model: 'codellama',
      });
      expect(result.success).toBe(true);
    });
  });

  describe('deleteOllamaModel', () => {
    it('calls DELETE /admin/ollama/models/:name', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteOllamaModel('llama3.2');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/ollama/models/llama3.2');
    });

    it('encodes special characters in model name', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteOllamaModel('codellama:13b');
      expect(apiClient.delete).toHaveBeenCalledWith('/admin/ollama/models/codellama%3A13b');
    });
  });

  describe('listCachedModels', () => {
    it('calls GET /admin/ollama/models', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([
        { id: 'm-1', model_name: 'llama3.2', size_bytes: 4100000000 },
      ]);
      const result = await listCachedModels();
      expect(apiClient.get).toHaveBeenCalledWith('/admin/ollama/models');
      expect(result).toHaveLength(1);
      expect(result[0].model_name).toBe('llama3.2');
    });
  });

  describe('formatModelSize', () => {
    it('returns dash for null', () => {
      expect(formatModelSize(null)).toBe('—');
    });

    it('returns dash for zero', () => {
      expect(formatModelSize(0)).toBe('—');
    });

    it('formats bytes', () => {
      expect(formatModelSize(500)).toBe('500.0 B');
    });

    it('formats kilobytes', () => {
      expect(formatModelSize(1024)).toBe('1.0 KB');
    });

    it('formats megabytes', () => {
      expect(formatModelSize(1048576)).toBe('1.0 MB');
    });

    it('formats gigabytes', () => {
      expect(formatModelSize(1073741824)).toBe('1.0 GB');
    });

    it('formats large model sizes', () => {
      // NOTE: Typical 7B parameter model size
      const result = formatModelSize(4100000000);
      expect(result).toContain('GB');
    });
  });
});
