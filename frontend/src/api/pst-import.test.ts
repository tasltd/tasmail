// Added: PST import API tests for TMAIL-115
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { pstImportApi } from './pst-import';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    delete: vi.fn(),
    getToken: vi.fn(),
  },
}));

// Added: Mock global fetch for multipart upload tests
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

describe('pst-import API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists PST imports via GET /migration/pst', async () => {
    const mockImports = [
      { id: '1', filename: 'outlook.pst', status: 'completed', messages_imported: 150 },
      { id: '2', filename: 'archive.pst', status: 'processing', messages_imported: 30 },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockImports);

    const result = await pstImportApi.list();

    expect(apiClient.get).toHaveBeenCalledWith('/migration/pst');
    expect(result).toHaveLength(2);
  });

  it('gets PST import by id', async () => {
    const mockImport = { id: 'pst-123', filename: 'mail.pst', status: 'completed' };
    vi.mocked(apiClient.get).mockResolvedValue(mockImport);

    const result = await pstImportApi.get('pst-123');

    expect(apiClient.get).toHaveBeenCalledWith('/migration/pst/pst-123');
    expect(result.status).toBe('completed');
  });

  it('uploads PST file via multipart POST', async () => {
    const mockFile = new File(['pst-data'], 'outlook.pst', { type: 'application/octet-stream' });
    const mockResponse = { id: 'new-import', filename: 'outlook.pst', status: 'pending' };

    vi.mocked(apiClient.getToken).mockReturnValue('test-jwt-token');
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    });

    const result = await pstImportApi.upload(mockFile, 'INBOX');

    expect(mockFetch).toHaveBeenCalledWith('/api/migration/pst/upload', expect.objectContaining({
      method: 'POST',
      headers: { Authorization: 'Bearer test-jwt-token' },
    }));
    expect(result.status).toBe('pending');
  });

  it('deletes PST import', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);

    await pstImportApi.delete('import-to-delete');

    expect(apiClient.delete).toHaveBeenCalledWith('/migration/pst/import-to-delete');
  });
});
