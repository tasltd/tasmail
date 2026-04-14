// Added: Webhook API tests for TMAIL-131

import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listWebhooks,
  createWebhook,
  getWebhook,
  updateWebhook,
  deleteWebhook,
  listDeliveries,
} from './webhooks';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('webhooks API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listWebhooks', () => {
    it('calls GET /webhooks', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listWebhooks();
      expect(apiClient.get).toHaveBeenCalledWith('/webhooks');
      expect(result).toEqual([]);
    });
  });

  describe('createWebhook', () => {
    it('calls POST /webhooks with full data', async () => {
      const webhookData = {
        url: 'https://example.com/hook',
        secret: 'my-secret',
        events: ['email.received' as const, 'email.sent' as const],
        description: 'Test webhook',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'wh-1', ...webhookData });

      const result = await createWebhook(webhookData);
      expect(apiClient.post).toHaveBeenCalledWith('/webhooks', webhookData);
      expect(result.url).toBe('https://example.com/hook');
    });
  });

  describe('getWebhook', () => {
    it('calls GET /webhooks/:id', async () => {
      const mockWebhook = { id: 'wh-1', url: 'https://example.com/hook' };
      vi.mocked(apiClient.get).mockResolvedValue(mockWebhook);

      const result = await getWebhook('wh-1');
      expect(apiClient.get).toHaveBeenCalledWith('/webhooks/wh-1');
      expect(result.id).toBe('wh-1');
    });
  });

  describe('updateWebhook', () => {
    it('calls PUT /webhooks/:id with partial update', async () => {
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'wh-1', active: false });

      await updateWebhook('wh-1', { active: false });
      expect(apiClient.put).toHaveBeenCalledWith('/webhooks/wh-1', { active: false });
    });
  });

  describe('deleteWebhook', () => {
    it('calls DELETE /webhooks/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteWebhook('wh-1');
      expect(apiClient.delete).toHaveBeenCalledWith('/webhooks/wh-1');
    });
  });

  describe('listDeliveries', () => {
    it('calls GET /webhooks/:id/deliveries', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listDeliveries('wh-1');
      expect(apiClient.get).toHaveBeenCalledWith('/webhooks/wh-1/deliveries');
      expect(result).toEqual([]);
    });
  });
});
