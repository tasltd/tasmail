import { describe, it, expect, vi, beforeEach } from 'vitest';
import { autoReplyApi } from './auto-reply';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    put: vi.fn(),
  },
}));

describe('auto-reply API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('fetches auto-reply rule', async () => {
    const mockRule = {
      id: '1',
      enabled: true,
      subject: 'Out of office',
      body_text: 'I am away',
    };
    vi.mocked(apiClient.get).mockResolvedValue(mockRule);

    const result = await autoReplyApi.get();
    expect(apiClient.get).toHaveBeenCalledWith('/auto-reply');
    expect(result).toEqual(mockRule);
  });

  it('returns null when no rule exists', async () => {
    vi.mocked(apiClient.get).mockResolvedValue(null);
    const result = await autoReplyApi.get();
    expect(result).toBeNull();
  });

  it('upserts auto-reply rule with all fields', async () => {
    const data = {
      enabled: true,
      subject: 'Out of office',
      body_text: 'I am on vacation',
      body_html: '<p>I am on vacation</p>',
      start_date: '2026-04-10',
      end_date: '2026-04-20',
      reply_to_all: false,
      exclude_lists: true,
    };
    vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...data });

    await autoReplyApi.set(data);
    expect(apiClient.put).toHaveBeenCalledWith('/auto-reply', data);
  });

  it('upserts minimal auto-reply rule', async () => {
    const data = {
      enabled: false,
      subject: 'Away',
      body_text: 'Gone',
    };
    vi.mocked(apiClient.put).mockResolvedValue({ id: '1', ...data });

    await autoReplyApi.set(data);
    expect(apiClient.put).toHaveBeenCalledWith('/auto-reply', data);
  });
});
