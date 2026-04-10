import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchFolders } from './folders';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
  },
}));

describe('folders API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('fetches folders from /folders endpoint', async () => {
    const mockFolders = [
      { name: 'INBOX', delimiter: '.', messages: 10, unseen: 3 },
      { name: 'Sent', delimiter: '.', messages: 5, unseen: 0 },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockFolders);

    const result = await fetchFolders();

    expect(apiClient.get).toHaveBeenCalledWith('/folders');
    expect(result).toEqual(mockFolders);
  });

  it('returns empty array when no folders', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    const result = await fetchFolders();
    expect(result).toEqual([]);
  });

  it('propagates API errors', async () => {
    vi.mocked(apiClient.get).mockRejectedValue(new Error('Network error'));
    await expect(fetchFolders()).rejects.toThrow('Network error');
  });
});
