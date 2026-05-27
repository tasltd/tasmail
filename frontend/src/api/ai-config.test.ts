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
  composeEmail,
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
    // Updated: TMAIL-103 — folder + uid now go on the wire so the backend can
    // hit its PostgreSQL summary cache instead of re-paying provider tokens.
    it('calls POST /ai/summarize with email text plus folder/uid for caching', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        summary: 'This email discusses the quarterly report.',
        provider: 'openai',
        model: 'gpt-4o',
        cached: false,
      });
      const result = await summarizeEmail('INBOX', 42, 'Hello, here is the quarterly report...');
      expect(apiClient.post).toHaveBeenCalledWith('/ai/summarize', {
        email_text: 'Hello, here is the quarterly report...',
        folder: 'INBOX',
        uid: 42,
      });
      expect(result.summary).toBe('This email discusses the quarterly report.');
      expect(result.cached).toBe(false);
    });

    // Added: TMAIL-103 — verify the cached flag surfaces when the backend
    // returns a hit, so the UI/telemetry can distinguish a cheap rerun.
    it('passes through cached=true on cache hits', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        summary: 'Cached summary.',
        provider: 'openai',
        model: 'gpt-4o',
        cached: true,
      });
      const result = await summarizeEmail('INBOX', 42, 'same content');
      expect(result.cached).toBe(true);
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

  // Added: Compose email API test for TMAIL-134
  describe('composeEmail', () => {
    it('calls POST /ai/compose with prompt and options', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        subject: 'Meeting Follow-Up',
        body: 'Hi team,\n\nJust following up on our discussion.',
        provider: 'openai',
        model: 'gpt-4o',
      });
      const result = await composeEmail(
        'Write a follow-up about the meeting',
        'We discussed deadlines',
        'professional',
        'medium',
      );
      expect(apiClient.post).toHaveBeenCalledWith('/ai/compose', {
        prompt: 'Write a follow-up about the meeting',
        context: 'We discussed deadlines',
        tone: 'professional',
        length: 'medium',
      });
      expect(result.subject).toBe('Meeting Follow-Up');
      expect(result.body).toContain('following up');
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
