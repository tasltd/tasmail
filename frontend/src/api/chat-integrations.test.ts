// Added: Chat integration API tests for TMAIL-129

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listChatIntegrations,
  createChatIntegration,
  getChatIntegration,
  updateChatIntegration,
  deleteChatIntegration,
  testChatIntegration,
} from './chat-integrations';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('chat-integrations API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listChatIntegrations', () => {
    it('calls GET /chat-integrations', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listChatIntegrations();
      expect(apiClient.get).toHaveBeenCalledWith('/chat-integrations');
      expect(result).toEqual([]);
    });
  });

  describe('createChatIntegration', () => {
    it('calls POST /chat-integrations with full data', async () => {
      const integrationData = {
        platform: 'slack' as const,
        webhook_url: 'https://hooks.slack.com/services/T00/B00/xxx',
        channel_name: '#general',
        notify_on_receive: true,
        notify_on_send: false,
        notify_on_mention: true,
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'ci-1', ...integrationData });

      const result = await createChatIntegration(integrationData);
      expect(apiClient.post).toHaveBeenCalledWith('/chat-integrations', integrationData);
      expect(result.platform).toBe('slack');
    });
  });

  describe('getChatIntegration', () => {
    it('calls GET /chat-integrations/:id', async () => {
      const mockIntegration = { id: 'ci-1', platform: 'slack' };
      vi.mocked(apiClient.get).mockResolvedValue(mockIntegration);

      const result = await getChatIntegration('ci-1');
      expect(apiClient.get).toHaveBeenCalledWith('/chat-integrations/ci-1');
      expect(result.id).toBe('ci-1');
    });
  });

  describe('updateChatIntegration', () => {
    it('calls PUT /chat-integrations/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'ci-1', active: false });

      await updateChatIntegration('ci-1', { active: false });
      expect(apiClient.put).toHaveBeenCalledWith('/chat-integrations/ci-1', { active: false });
    });
  });

  describe('deleteChatIntegration', () => {
    it('calls DELETE /chat-integrations/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteChatIntegration('ci-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/chat-integrations/ci-1');
    });
  });

  describe('testChatIntegration', () => {
    it('calls POST /chat-integrations/:id/test', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({ success: true, message: 'Test notification sent successfully' });
      const result = await testChatIntegration('ci-1');
      expect(apiClient.post).toHaveBeenCalledWith('/chat-integrations/ci-1/test', {});
      expect(result.success).toBe(true);
    });
  });
});
