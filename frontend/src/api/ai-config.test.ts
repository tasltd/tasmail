// Added: AI configuration API tests for TMAIL-105

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listAiConfigs,
  createAiConfig,
  updateAiConfig,
  deleteAiConfig,
  testAiConfig,
  summarizeEmail,
  summarizeThread,
  getSmartReply,
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
    // Added: Updated test to include folder and uid params (TMAIL-103)
    it('calls POST /ai/summarize with email text', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        summary: 'This email discusses the quarterly report.',
        provider: 'openai',
        model: 'gpt-4o',
      });
      const result = await summarizeEmail('INBOX', 42, 'Hello, here is the quarterly report...');
      expect(apiClient.post).toHaveBeenCalledWith('/ai/summarize', {
        email_text: 'Hello, here is the quarterly report...',
      });
      expect(result.summary).toBe('This email discusses the quarterly report.');
    });
  });

  // Added: Thread summarization API test for TMAIL-103
  describe('summarizeThread', () => {
    it('calls POST /ai/thread-summary with folder and uids', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        summary: 'The thread discusses project deadlines and task assignments.',
        message_count: 3,
        provider: 'anthropic',
        model: 'claude-sonnet-4-20250514',
      });
      const result = await summarizeThread('INBOX', [10, 11, 12]);
      expect(apiClient.post).toHaveBeenCalledWith('/ai/thread-summary', {
        folder: 'INBOX',
        uids: [10, 11, 12],
      });
      expect(result.summary).toBe('The thread discusses project deadlines and task assignments.');
      expect(result.message_count).toBe(3);
    });
  });

  // Added: Smart reply API test for TMAIL-104
  describe('getSmartReply', () => {
    it('calls POST /ai/smart-reply with folder, uid, and tone', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        reply: 'Thank you for your message. I will review the report.',
        tone: 'brief',
        provider: 'openai',
        model: 'gpt-4o',
      });
      const result = await getSmartReply('INBOX', 42, 'brief');
      expect(apiClient.post).toHaveBeenCalledWith('/ai/smart-reply', {
        folder: 'INBOX',
        uid: 42,
        tone: 'brief',
      });
      expect(result.reply).toBe('Thank you for your message. I will review the report.');
      expect(result.tone).toBe('brief');
    });
  });
});
