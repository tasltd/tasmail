// Added: AI configuration API tests for TMAIL-105

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listAiConfigs,
  createAiConfig,
  updateAiConfig,
  deleteAiConfig,
  testAiConfig,
  summarizeEmail,
} from './ai-config';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('ai-config API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listAiConfigs', () => {
    it('calls GET /ai/config', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listAiConfigs();
      expect(apiClient.get).toHaveBeenCalledWith('/ai/config');
      expect(result).toEqual([]);
    });
  });

  describe('createAiConfig', () => {
    it('calls POST /ai/config with full data', async () => {
      const configData = {
        provider: 'openai' as const,
        api_key: 'sk-test-key-12345',
        model_name: 'gpt-4o',
        max_tokens: 1000,
        temperature: 0.5,
      };
      vi.mocked(apiClient.post).mockResolvedValue({
        id: 'ai-1',
        provider: 'openai',
        api_key_masked: 'sk-t...2345',
        model_name: 'gpt-4o',
      });

      const result = await createAiConfig(configData);
      expect(apiClient.post).toHaveBeenCalledWith('/ai/config', configData);
      expect(result.provider).toBe('openai');
    });
  });

  describe('updateAiConfig', () => {
    it('calls PUT /ai/config/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'ai-1', active: false });

      await updateAiConfig('ai-1', { active: false });
      expect(apiClient.put).toHaveBeenCalledWith('/ai/config/ai-1', { active: false });
    });
  });

  describe('deleteAiConfig', () => {
    it('calls DELETE /ai/config/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteAiConfig('ai-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/ai/config/ai-1');
    });
  });

  describe('testAiConfig', () => {
    it('calls POST /ai/config/:id/test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        success: true,
        message: 'API key verified successfully',
        response: 'Connection successful',
      });
      const result = await testAiConfig('ai-1');
      expect(apiClient.post).toHaveBeenCalledWith('/ai/config/ai-1/test', {});
      expect(result.success).toBe(true);
    });
  });

  describe('summarizeEmail', () => {
    it('calls POST /ai/summarize with email text', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        summary: 'This email discusses the quarterly report.',
        provider: 'openai',
        model: 'gpt-4o',
      });
      const result = await summarizeEmail('Hello, here is the quarterly report...');
      expect(apiClient.post).toHaveBeenCalledWith('/ai/summarize', {
        email_text: 'Hello, here is the quarterly report...',
      });
      expect(result.summary).toBe('This email discusses the quarterly report.');
    });
  });
});
