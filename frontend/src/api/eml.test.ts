// Added: Unit tests for EML import/export API functions (TMAIL-68)
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { exportEml, importEml, downloadEml } from './eml';

// NOTE: We mock global fetch since EML operations use raw fetch (not apiClient)
// for binary request/response handling
const mockFetch = vi.fn();

beforeEach(() => {
  mockFetch.mockClear();
  vi.stubGlobal('fetch', mockFetch);
  // Added: Provide a mock access_token in localStorage
  localStorage.setItem('access_token', 'test-jwt-token');
});

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe('exportEml', () => {
  it('calls GET with correct URL and auth header', async () => {
    const mockBlob = new Blob(['fake-eml-content'], { type: 'message/rfc822' });
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    const result = await exportEml('INBOX', 42);

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/folders/INBOX/messages/42/eml',
      {
        headers: { Authorization: 'Bearer test-jwt-token' },
      },
    );
    expect(result).toBe(mockBlob);
  });

  it('encodes folder names with special characters', async () => {
    const mockBlob = new Blob(['content']);
    mockFetch.mockResolvedValue({
      ok: true,
      blob: () => Promise.resolve(mockBlob),
    });

    await exportEml('Sent Items', 10);

    expect(mockFetch).toHaveBeenCalledWith(
      '/api/folders/Sent%20Items/messages/10/eml',
      expect.any(Object),
    );
  });

  it('throws error with details on failed response', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
      text: () => Promise.resolve('{"error":"Message not found"}'),
    });

    await expect(exportEml('INBOX', 999)).rejects.toThrow(
      "EML export failed for UID 999 in folder 'INBOX': 404",
    );
  });
});

describe('importEml', () => {
  it('calls POST with correct URL, headers, and body', async () => {
    mockFetch.mockResolvedValue({ ok: true });
    const fileContent = 'From: test@example.com\r\nSubject: Test\r\n\r\nBody';
    const mockFile = new File([fileContent], 'test.eml', { type: 'message/rfc822' });

    await importEml('INBOX', mockFile);

    expect(mockFetch).toHaveBeenCalledTimes(1);
    const [calledUrl, calledOptions] = mockFetch.mock.calls[0];
    expect(calledUrl).toBe('/api/folders/INBOX/import-eml');
    expect(calledOptions.method).toBe('POST');
    expect(calledOptions.headers['Content-Type']).toBe('message/rfc822');
    expect(calledOptions.headers['Authorization']).toBe('Bearer test-jwt-token');
    // Added: Verify body is an ArrayBuffer (binary data)
    expect(calledOptions.body).toBeInstanceOf(ArrayBuffer);
  });

  it('encodes folder names in URL', async () => {
    mockFetch.mockResolvedValue({ ok: true });
    const mockFile = new File(['content'], 'email.eml');

    await importEml('My Folder', mockFile);

    const calledUrl = mockFetch.mock.calls[0][0];
    expect(calledUrl).toBe('/api/folders/My%20Folder/import-eml');
  });

  it('throws error with details on failed response', async () => {
    mockFetch.mockResolvedValue({
      ok: false,
      status: 502,
      text: () => Promise.resolve('{"error":"IMAP APPEND failed"}'),
    });
    const mockFile = new File(['content'], 'bad.eml');

    await expect(importEml('INBOX', mockFile)).rejects.toThrow(
      "EML import failed for folder 'INBOX': 502",
    );
  });
});

describe('downloadEml', () => {
  it('creates anchor element with correct href and download attribute', () => {
    // Added: Mock URL.createObjectURL and URL.revokeObjectURL
    const mockObjectUrl = 'blob:http://localhost/fake-uuid';
    const createObjectUrlSpy = vi.fn(() => mockObjectUrl);
    const revokeObjectUrlSpy = vi.fn();
    vi.stubGlobal('URL', {
      ...URL,
      createObjectURL: createObjectUrlSpy,
      revokeObjectURL: revokeObjectUrlSpy,
    });

    // Added: Spy on document.createElement to capture the anchor element
    const mockAnchor = { href: '', download: '', click: vi.fn() };
    const createElementSpy = vi.spyOn(document, 'createElement').mockReturnValue(mockAnchor as unknown as HTMLElement);

    const blob = new Blob(['eml-content'], { type: 'message/rfc822' });
    downloadEml(blob, 55);

    expect(createObjectUrlSpy).toHaveBeenCalledWith(blob);
    expect(mockAnchor.href).toBe(mockObjectUrl);
    expect(mockAnchor.download).toBe('message_55.eml');
    expect(mockAnchor.click).toHaveBeenCalledOnce();
    expect(revokeObjectUrlSpy).toHaveBeenCalledWith(mockObjectUrl);

    createElementSpy.mockRestore();
  });

  it('generates correct filename for different UIDs', () => {
    const mockObjectUrl = 'blob:http://localhost/test';
    vi.stubGlobal('URL', {
      ...URL,
      createObjectURL: vi.fn(() => mockObjectUrl),
      revokeObjectURL: vi.fn(),
    });

    const mockAnchor = { href: '', download: '', click: vi.fn() };
    const createElementSpy = vi.spyOn(document, 'createElement').mockReturnValue(mockAnchor as unknown as HTMLElement);

    const blob = new Blob(['content']);

    downloadEml(blob, 1);
    expect(mockAnchor.download).toBe('message_1.eml');

    downloadEml(blob, 99999);
    expect(mockAnchor.download).toBe('message_99999.eml');

    createElementSpy.mockRestore();
  });
});
