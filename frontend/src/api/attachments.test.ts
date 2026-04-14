// Added: Unit tests for attachment API module — TMAIL-59
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { attachmentsApi } from './attachments';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
    getToken: vi.fn(() => 'test-token'),
  },
}));

// Added: Mock fetch globally for upload/download tests that bypass apiClient
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

describe('attachments API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists attachments via GET /attachments', async () => {
    const mockAttachments = [
      {
        id: 'att-1',
        mailbox_id: 'mb-1',
        filename: 'report.pdf',
        content_type: 'application/pdf',
        size_bytes: 1024000,
        scan_status: 'clean',
        created_at: '2026-04-14T10:00:00Z',
      },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockAttachments);

    const result = await attachmentsApi.list();

    expect(apiClient.get).toHaveBeenCalledWith('/attachments');
    expect(result).toHaveLength(1);
    expect(result[0].filename).toBe('report.pdf');
  });

  it('deletes attachment via DELETE /attachments/:id', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);

    await attachmentsApi.delete('att-123');

    expect(apiClient.delete).toHaveBeenCalledWith('/attachments/att-123');
  });

  it('gets storage stats via GET /attachments/stats', async () => {
    const mockStats = {
      total_count: 10,
      total_size_bytes: 52428800,
      pending_scans: 1,
      infected_count: 0,
    };
    vi.mocked(apiClient.get).mockResolvedValue(mockStats);

    const result = await attachmentsApi.stats();

    expect(apiClient.get).toHaveBeenCalledWith('/attachments/stats');
    expect(result.total_count).toBe(10);
    expect(result.total_size_bytes).toBe(52428800);
  });

  it('uploads file via multipart POST /attachments', async () => {
    const mockAttachment = {
      id: 'att-new',
      filename: 'test.txt',
      scan_status: 'pending',
    };
    mockFetch.mockResolvedValue({
      ok: true,
      json: () => Promise.resolve(mockAttachment),
    });

    const file = new File(['hello'], 'test.txt', { type: 'text/plain' });
    const result = await attachmentsApi.upload(file);

    expect(mockFetch).toHaveBeenCalledTimes(1);
    // Added: Verify POST method and Authorization header
    const [url, options] = mockFetch.mock.calls[0];
    expect(url).toContain('/attachments');
    expect(options.method).toBe('POST');
    expect(options.headers['Authorization']).toBe('Bearer test-token');
    expect(result.id).toBe('att-new');
  });

  it('throws on upload failure', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 413,
      text: () => Promise.resolve('File too large'),
    });

    const file = new File(['data'], 'big.zip', { type: 'application/zip' });

    await expect(attachmentsApi.upload(file)).rejects.toThrow('Upload failed (413)');
  });

  it('downloads attachment as blob', async () => {
    const mockBlob = new Blob(['file-content'], { type: 'application/pdf' });
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    const result = await attachmentsApi.download('att-456');

    expect(mockFetch).toHaveBeenCalledTimes(1);
    const [url, options] = mockFetch.mock.calls[0];
    expect(url).toContain('/attachments/att-456/download');
    expect(options.headers['Authorization']).toBe('Bearer test-token');
    expect(result).toBe(mockBlob);
  });

  it('throws on download failure', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
      text: () => Promise.resolve('Not found'),
    });

    await expect(attachmentsApi.download('nonexistent')).rejects.toThrow(
      'Download failed (404)',
    );
  });

  it('includes correct stats structure', async () => {
    const mockStats = {
      total_count: 0,
      total_size_bytes: 0,
      pending_scans: 0,
      infected_count: 0,
    };
    vi.mocked(apiClient.get).mockResolvedValue(mockStats);

    const result = await attachmentsApi.stats();

    expect(result.pending_scans).toBe(0);
    expect(result.infected_count).toBe(0);
  });
});
