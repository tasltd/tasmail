// Added: Unit tests for shared files API module (TMAIL-138)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listSharedFiles, getSharedFile, deleteSharedFile, getDownloadUrl, uploadSharedFile } from './shared-files';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
    getToken: vi.fn(),
  },
}));

// Added: Mock fetch for the upload function which uses fetch directly
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

describe('shared-files API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listSharedFiles', () => {
    it('calls GET /shared-files', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listSharedFiles();
      expect(apiClient.get).toHaveBeenCalledWith('/shared-files');
      expect(result).toEqual([]);
    });
  });

  describe('getSharedFile', () => {
    it('calls GET /shared-files/:id', async () => {
      const mockFile = { id: 'abc-123', filename: 'test.pdf' };
      vi.mocked(apiClient.get).mockResolvedValue(mockFile);
      const result = await getSharedFile('abc-123');
      expect(apiClient.get).toHaveBeenCalledWith('/shared-files/abc-123');
      expect(result).toEqual(mockFile);
    });
  });

  describe('deleteSharedFile', () => {
    it('calls DELETE /shared-files/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteSharedFile('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/shared-files/abc-123');
    });
  });

  describe('getDownloadUrl', () => {
    it('generates correct public download URL from token', () => {
      const url = getDownloadUrl('abc123def456');
      expect(url).toContain('/dl/abc123def456');
    });
  });

  describe('uploadSharedFile', () => {
    it('sends FormData via fetch with auth header', async () => {
      vi.mocked(apiClient.getToken).mockReturnValue('test-token');
      mockFetch.mockResolvedValue({
        ok: true,
        json: () => Promise.resolve({ id: 'new-id', filename: 'upload.pdf' }),
      });

      const formData = new FormData();
      formData.append('file', new Blob(['test']), 'upload.pdf');

      const result = await uploadSharedFile(formData);
      expect(mockFetch).toHaveBeenCalledWith(
        expect.stringContaining('/shared-files/upload'),
        expect.objectContaining({
          method: 'POST',
          headers: { Authorization: 'Bearer test-token' },
          body: formData,
        }),
      );
      expect(result.filename).toBe('upload.pdf');
    });
  });
});
