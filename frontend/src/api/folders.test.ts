import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchFolders, createFolder, deleteFolder } from './folders';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
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

  // TMAIL-324: createFolder POSTs to /folders with the user-supplied name and
  // returns the server's canonical Folder shape so the caller can append it
  // optimistically (or invalidate the ['folders'] query to refetch).
  describe('createFolder', () => {
    it('POSTs /folders with {name} body and returns the created folder', async () => {
      const created = { name: 'Projects', delimiter: '/', messages: 0, unseen: 0 };
      vi.mocked(apiClient.post).mockResolvedValue(created);

      const result = await createFolder('Projects');

      expect(apiClient.post).toHaveBeenCalledWith('/folders', { name: 'Projects' });
      expect(result).toEqual(created);
    });

    it('forwards the exact name without trimming or modification', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({
        name: '  Spaced  ',
        delimiter: '/',
        messages: 0,
        unseen: 0,
      });

      await createFolder('  Spaced  ');
      expect(apiClient.post).toHaveBeenCalledWith('/folders', { name: '  Spaced  ' });
    });

    it('propagates server-side validation errors (e.g. built-in folder)', async () => {
      vi.mocked(apiClient.post).mockRejectedValue(
        new Error('API Error 400: INBOX is a built-in folder'),
      );
      await expect(createFolder('INBOX')).rejects.toThrow('built-in folder');
    });
  });

  // TMAIL-324: deleteFolder issues DELETE to the URL-encoded folder name and
  // expects a 204 (handled by ApiClient as undefined).
  describe('deleteFolder', () => {
    it('DELETEs /folders/{name} with the folder name URL-encoded', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteFolder('Projects');

      expect(apiClient.delete).toHaveBeenCalledWith('/folders/Projects');
    });

    it('URL-encodes special characters in folder names', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteFolder('Work & Personal');

      expect(apiClient.delete).toHaveBeenCalledWith('/folders/Work%20%26%20Personal');
    });

    it('propagates server-side errors (e.g. attempting to delete INBOX)', async () => {
      vi.mocked(apiClient.delete).mockRejectedValue(
        new Error('API Error 400: INBOX cannot be deleted'),
      );
      await expect(deleteFolder('INBOX')).rejects.toThrow('cannot be deleted');
    });
  });
});
