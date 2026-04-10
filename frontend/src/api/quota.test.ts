import { describe, it, expect, vi, beforeEach } from 'vitest';
import { quotaApi } from './quota';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
  },
}));

describe('quota API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('gets quota status via GET /quota', async () => {
    const mockQuota = {
      mailbox_id: 'test-id',
      quota_bytes: 1073741824,
      used_bytes: 536870912,
      message_count: 150,
      usage_percent: 50.0,
      quota_warn_percent: 80,
      is_over_quota: false,
      is_warning: false,
      last_synced_at: '2026-04-10T00:00:00Z',
    };
    vi.mocked(apiClient.get).mockResolvedValue(mockQuota);

    const result = await quotaApi.getQuota();

    expect(apiClient.get).toHaveBeenCalledWith('/quota');
    expect(result.usage_percent).toBe(50.0);
    expect(result.is_over_quota).toBe(false);
  });

  it('syncs quota via POST /quota/sync', async () => {
    const mockQuota = {
      mailbox_id: 'test-id',
      quota_bytes: 1073741824,
      used_bytes: 900000000,
      message_count: 500,
      usage_percent: 83.8,
      quota_warn_percent: 80,
      is_over_quota: false,
      is_warning: true,
      last_synced_at: '2026-04-10T12:00:00Z',
    };
    vi.mocked(apiClient.post).mockResolvedValue(mockQuota);

    const result = await quotaApi.syncQuota();

    expect(apiClient.post).toHaveBeenCalledWith('/quota/sync');
    expect(result.is_warning).toBe(true);
  });
});
