// Added: Bulk import API tests for TMAIL-136
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { bulkImportApi } from './bulk-import';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    getToken: vi.fn(),
  },
}));

// Added: Mock global fetch for multipart upload and template download tests
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

describe('bulk-import API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists bulk imports via GET /admin/users/bulk-imports', async () => {
    const mockImports = [
      { id: '1', filename: 'users.csv', status: 'completed', success_count: 10 },
      { id: '2', filename: 'staff.csv', status: 'failed', error_count: 3 },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockImports);

    const result = await bulkImportApi.list();

    expect(apiClient.get).toHaveBeenCalledWith('/admin/users/bulk-imports');
    expect(result).toHaveLength(2);
  });

  it('gets bulk import by id', async () => {
    const mockImport = { id: 'import-123', filename: 'users.csv', status: 'completed' };
    vi.mocked(apiClient.get).mockResolvedValue(mockImport);

    const result = await bulkImportApi.get('import-123');

    expect(apiClient.get).toHaveBeenCalledWith('/admin/users/bulk-imports/import-123');
    expect(result.status).toBe('completed');
  });

  it('uploads CSV file via multipart POST', async () => {
    const mockFile = new File(['email,display_name,password,role'], 'users.csv', {
      type: 'text/csv',
    });
    const mockResponse = { id: 'new-import', filename: 'users.csv', status: 'completed' };

    vi.mocked(apiClient.getToken).mockReturnValue('admin-jwt-token');
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockResponse),
    });

    const result = await bulkImportApi.upload(mockFile);

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/admin/users/bulk-import',
      expect.objectContaining({
        method: 'POST',
        headers: { Authorization: 'Bearer admin-jwt-token' },
      }),
    );
    expect(result.status).toBe('completed');
  });

  it('downloads template via GET with browser download', async () => {
    vi.mocked(apiClient.getToken).mockReturnValue('admin-jwt-token');

    // Added: Mock blob response for template download
    const mockBlob = new Blob(['email,display_name,password,role'], { type: 'text/csv' });
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    // Added: Mock URL.createObjectURL and anchor click
    const mockCreateObjectURL = vi.fn().mockReturnValue('blob:mock-url');
    const mockRevokeObjectURL = vi.fn();
    vi.stubGlobal('URL', { createObjectURL: mockCreateObjectURL, revokeObjectURL: mockRevokeObjectURL });

    const mockClick = vi.fn();
    vi.spyOn(document, 'createElement').mockReturnValue({
      set href(_: string) {},
      set download(_: string) {},
      click: mockClick,
    } as unknown as HTMLElement);

    await bulkImportApi.downloadTemplate();

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/admin/users/bulk-import/template',
      expect.objectContaining({
        headers: { Authorization: 'Bearer admin-jwt-token' },
      }),
    );
    expect(mockClick).toHaveBeenCalled();
  });

  // Added: exportUsers tests (TMAIL-136 — companion to bulk-import)
  it('exports users via GET /api/admin/users/export with browser download', async () => {
    vi.mocked(apiClient.getToken).mockReturnValue('admin-jwt-token');

    const exportPayload = 'email,display_name,role,active,quota_bytes,created_at\nalice@example.com,Alice,user,true,5368709120,2026-04-15T10:30:00+00:00';
    const mockBlob = new Blob([exportPayload], { type: 'text/csv' });
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    const mockCreateObjectURL = vi.fn().mockReturnValue('blob:export-url');
    const mockRevokeObjectURL = vi.fn();
    vi.stubGlobal('URL', { createObjectURL: mockCreateObjectURL, revokeObjectURL: mockRevokeObjectURL });

    const mockClick = vi.fn();
    let capturedDownloadName: string | undefined;
    vi.spyOn(document, 'createElement').mockReturnValue({
      set href(_: string) {},
      set download(name: string) {
        capturedDownloadName = name;
      },
      click: mockClick,
    } as unknown as HTMLElement);

    await bulkImportApi.exportUsers();

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/admin/users/export',
      expect.objectContaining({
        headers: { Authorization: 'Bearer admin-jwt-token' },
      }),
    );
    expect(mockClick).toHaveBeenCalled();
    expect(capturedDownloadName).toBe('users-export.csv');
    expect(mockRevokeObjectURL).toHaveBeenCalledWith('blob:export-url');
  });

  it('throws an error when export request fails', async () => {
    vi.mocked(apiClient.getToken).mockReturnValue('admin-jwt-token');
    mockFetch.mockResolvedValue({
      ok: false,
      status: 403,
      text: () => Promise.resolve('Admin access required'),
    });

    await expect(bulkImportApi.exportUsers()).rejects.toThrow(/Export failed \(403\): Admin access required/);
  });

  it('sends export request without auth header when no token is present', async () => {
    vi.mocked(apiClient.getToken).mockReturnValue(null as unknown as string);

    const mockBlob = new Blob(['email,display_name,role,active,quota_bytes,created_at'], { type: 'text/csv' });
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    vi.stubGlobal('URL', {
      createObjectURL: vi.fn().mockReturnValue('blob:no-token-url'),
      revokeObjectURL: vi.fn(),
    });
    vi.spyOn(document, 'createElement').mockReturnValue({
      set href(_: string) {},
      set download(_: string) {},
      click: vi.fn(),
    } as unknown as HTMLElement);

    await bulkImportApi.exportUsers();

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/admin/users/export',
      expect.objectContaining({ headers: {} }),
    );
  });
});
