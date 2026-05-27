// Added: Unit tests for shared files API module (TMAIL-138)
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  listSharedFiles,
  getSharedFile,
  deleteSharedFile,
  getDownloadUrl,
  uploadSharedFile,
  uploadSharedFileWithProgress,
} from './shared-files';
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

  // Added: TMAIL-138 — tests for XHR-based uploader that emits progress events.
  describe('uploadSharedFileWithProgress', () => {
    // Added: Minimal XHR stub that captures handlers + supports simulated events.
    interface StubXhr {
      open: ReturnType<typeof vi.fn>;
      send: ReturnType<typeof vi.fn>;
      setRequestHeader: ReturnType<typeof vi.fn>;
      upload: { onprogress: ((ev: ProgressEvent) => void) | null };
      onload: (() => void) | null;
      onerror: (() => void) | null;
      onabort: (() => void) | null;
      status: number;
      responseText: string;
    }
    let stub: StubXhr;
    let originalXhr: typeof globalThis.XMLHttpRequest;

    beforeEach(() => {
      vi.mocked(apiClient.getToken).mockReturnValue('test-token');
      stub = {
        open: vi.fn(),
        send: vi.fn(),
        setRequestHeader: vi.fn(),
        upload: { onprogress: null },
        onload: null,
        onerror: null,
        onabort: null,
        status: 0,
        responseText: '',
      };
      originalXhr = globalThis.XMLHttpRequest;
      // NOTE: Cast through unknown to satisfy DOM typings for our minimal stub.
      (globalThis as unknown as { XMLHttpRequest: unknown }).XMLHttpRequest =
        function XMLHttpRequest() { return stub; } as unknown;
    });

    afterEach(() => {
      (globalThis as unknown as { XMLHttpRequest: unknown }).XMLHttpRequest = originalXhr;
    });

    it('resolves with parsed JSON and reports progress events', async () => {
      const onProgress = vi.fn();
      const formData = new FormData();
      formData.append('file', new Blob(['hi']), 'a.txt');

      const promise = uploadSharedFileWithProgress(formData, onProgress);

      // simulate progress event
      expect(stub.upload.onprogress).not.toBeNull();
      stub.upload.onprogress!({ lengthComputable: true, loaded: 50, total: 100 } as ProgressEvent);
      expect(onProgress).toHaveBeenCalledWith({ loaded: 50, total: 100 });

      // simulate successful completion
      stub.status = 201;
      stub.responseText = JSON.stringify({ id: 'id-1', filename: 'a.txt' });
      stub.onload!();

      const result = await promise;
      expect(result).toMatchObject({ id: 'id-1', filename: 'a.txt' });
      expect(stub.setRequestHeader).toHaveBeenCalledWith('Authorization', 'Bearer test-token');
      expect(stub.open).toHaveBeenCalledWith('POST', expect.stringContaining('/shared-files/upload'));
    });

    it('rejects with the server error body when status is non-2xx', async () => {
      const formData = new FormData();
      formData.append('file', new Blob(['hi']), 'a.txt');
      const promise = uploadSharedFileWithProgress(formData);

      stub.status = 500;
      stub.responseText = 'boom';
      stub.onload!();

      await expect(promise).rejects.toThrow(/Upload failed \(500\): boom/);
    });

    it('rejects on network error', async () => {
      const formData = new FormData();
      formData.append('file', new Blob(['hi']), 'a.txt');
      const promise = uploadSharedFileWithProgress(formData);

      stub.onerror!();

      await expect(promise).rejects.toThrow(/Network error/);
    });
  });
});
