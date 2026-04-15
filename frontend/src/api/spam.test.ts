// Added: Unit tests for Rspamd spam filter API client (TMAIL-15)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  fetchSpamSettings,
  updateSpamSettings,
  fetchQuarantine,
  releaseQuarantine,
  deleteQuarantine,
  learnMessage,
  fetchSpamStats,
} from './spam';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('spam API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchSpamSettings', () => {
    it('calls GET /spam/settings', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(null);
      const result = await fetchSpamSettings();
      expect(apiClient.get).toHaveBeenCalledWith('/spam/settings');
      expect(result).toBeNull();
    });

    it('returns settings when available', async () => {
      const mockSettings = {
        id: 'abc',
        threshold_reject: 15.0,
        threshold_greylist: 4.0,
        threshold_add_header: 6.0,
        dkim_signing_enabled: true,
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockSettings);
      const result = await fetchSpamSettings();
      expect(result).toEqual(mockSettings);
    });
  });

  describe('updateSpamSettings', () => {
    it('calls PUT /spam/settings with data', async () => {
      const data = { threshold_reject: 20.0, dkim_signing_enabled: false };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', ...data });
      const result = await updateSpamSettings(data);
      expect(apiClient.put).toHaveBeenCalledWith('/spam/settings', data);
      expect(result).toHaveProperty('threshold_reject', 20.0);
    });
  });

  describe('fetchQuarantine', () => {
    it('calls GET /spam/quarantine', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await fetchQuarantine();
      expect(apiClient.get).toHaveBeenCalledWith('/spam/quarantine');
      expect(result).toEqual([]);
    });

    it('returns quarantined items', async () => {
      const items = [
        { id: '1', sender: 'spam@bad.com', subject: 'Buy now', score: 15.5, action: 'reject' },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(items);
      const result = await fetchQuarantine();
      expect(result).toHaveLength(1);
      expect(result[0].score).toBe(15.5);
    });
  });

  describe('releaseQuarantine', () => {
    it('calls POST /spam/quarantine/:id/release', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await releaseQuarantine('item-123');
      expect(apiClient.post).toHaveBeenCalledWith('/spam/quarantine/item-123/release');
    });
  });

  describe('deleteQuarantine', () => {
    it('calls DELETE /spam/quarantine/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteQuarantine('item-456');
      expect(apiClient.delete).toHaveBeenCalledWith('/spam/quarantine/item-456');
    });
  });

  describe('learnMessage', () => {
    it('calls POST /spam/learn with spam data', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await learnMessage({ message_id: 'msg-1', folder: 'INBOX', is_spam: true });
      expect(apiClient.post).toHaveBeenCalledWith('/spam/learn', {
        message_id: 'msg-1',
        folder: 'INBOX',
        is_spam: true,
      });
    });

    it('calls POST /spam/learn with ham data', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);
      await learnMessage({ message_id: 'msg-2', folder: 'Spam', is_spam: false });
      expect(apiClient.post).toHaveBeenCalledWith('/spam/learn', {
        message_id: 'msg-2',
        folder: 'Spam',
        is_spam: false,
      });
    });
  });

  describe('fetchSpamStats', () => {
    it('calls GET /spam/stats and returns stats', async () => {
      const mockStats = {
        total_scanned: 5000,
        total_blocked: 500,
        total_passed: 4500,
        quarantined: 100,
        released: 20,
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockStats);
      const result = await fetchSpamStats();
      expect(apiClient.get).toHaveBeenCalledWith('/spam/stats');
      expect(result).toEqual(mockStats);
    });
  });
});
