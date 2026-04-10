import { describe, it, expect, vi, beforeEach } from 'vitest';
import { scheduledApi } from './scheduled';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
  },
}));

describe('scheduled API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('scheduleSend', () => {
    it('calls POST /messages/schedule with delay', async () => {
      const req = {
        to: ['user@example.com'],
        subject: 'Test',
        text_body: 'Hello',
        delay_seconds: 10,
      };
      const mockResponse = {
        id: '1',
        cancel_token: 'token-123',
        scheduled_at: '2026-04-10T12:00:00Z',
        can_undo_until: '2026-04-10T12:00:10Z',
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

      const result = await scheduledApi.scheduleSend(req);
      expect(apiClient.post).toHaveBeenCalledWith('/messages/schedule', req);
      expect(result.cancel_token).toBe('token-123');
    });

    it('calls POST /messages/schedule with scheduled_at', async () => {
      const req = {
        to: ['user@example.com'],
        subject: 'Scheduled',
        scheduled_at: '2026-04-15T09:00:00Z',
      };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', cancel_token: 'abc' });

      await scheduledApi.scheduleSend(req);
      expect(apiClient.post).toHaveBeenCalledWith('/messages/schedule', req);
    });
  });

  describe('cancelScheduled', () => {
    it('calls POST /messages/cancel/:token', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await scheduledApi.cancelScheduled('token-123');
      expect(apiClient.post).toHaveBeenCalledWith('/messages/cancel/token-123');
    });
  });

  describe('listScheduled', () => {
    it('calls GET /messages/scheduled without filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await scheduledApi.listScheduled();
      expect(apiClient.get).toHaveBeenCalledWith('/messages/scheduled');
    });

    it('calls GET /messages/scheduled with status filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await scheduledApi.listScheduled('pending');
      expect(apiClient.get).toHaveBeenCalledWith('/messages/scheduled?status=pending');
    });
  });
});
