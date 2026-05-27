// Added: Unit tests for LargeFileAttacher component + helpers (TMAIL-138)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { LargeFileAttacher } from './LargeFileAttacher';
import {
  formatBytes,
  isLargeFile,
  buildSharedFileLinkHtml,
  computeExpiresAt,
} from './large-file-attacher-utils';
import type { SharedFile } from '../../api/shared-files';
import { LARGE_FILE_THRESHOLD_BYTES, MAX_SHARED_FILE_BYTES } from '../../utils/constants';

const mockUploadWithProgress = vi.fn();
vi.mock('../../api/shared-files', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../api/shared-files')>();
  return {
    ...actual,
    uploadSharedFileWithProgress: (
      ...args: Parameters<typeof actual.uploadSharedFileWithProgress>
    ) => mockUploadWithProgress(...args),
    getDownloadUrl: (token: string) => `http://localhost/api/dl/${token}`,
  };
});

const sampleSharedFile: SharedFile = {
  id: 'file-1',
  user_id: 'user-1',
  filename: 'big-deck.pdf',
  content_type: 'application/pdf',
  file_size: 60 * 1024 * 1024,
  storage_path: '/data/shared/big-deck.pdf',
  download_token: 'tok-abc',
  download_count: 0,
  max_downloads: null,
  expires_at: '2027-01-01T00:00:00.000Z',
  password_hash: null,
  created_at: '2026-05-27T10:00:00Z',
};

describe('LargeFileAttacher helpers', () => {
  describe('formatBytes', () => {
    it('formats bytes/KB/MB/GB across boundaries', () => {
      expect(formatBytes(0)).toBe('0 B');
      expect(formatBytes(1023)).toBe('1023 B');
      expect(formatBytes(1024)).toBe('1.0 KB');
      expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
      expect(formatBytes(2.5 * 1024 * 1024 * 1024)).toBe('2.5 GB');
    });
  });

  describe('isLargeFile', () => {
    it('returns false at and below the threshold', () => {
      expect(isLargeFile(LARGE_FILE_THRESHOLD_BYTES)).toBe(false);
      expect(isLargeFile(LARGE_FILE_THRESHOLD_BYTES - 1)).toBe(false);
    });
    it('returns true above the threshold', () => {
      expect(isLargeFile(LARGE_FILE_THRESHOLD_BYTES + 1)).toBe(true);
    });
    it('respects a custom threshold override', () => {
      expect(isLargeFile(1024, 2048)).toBe(false);
      expect(isLargeFile(4096, 2048)).toBe(true);
    });
  });

  describe('computeExpiresAt', () => {
    it('returns null for the Never option', () => {
      expect(computeExpiresAt(null)).toBeNull();
    });
    it('adds N days to the provided clock', () => {
      const fixed = new Date('2026-01-01T00:00:00Z');
      expect(computeExpiresAt(7, fixed)).toBe('2026-01-08T00:00:00.000Z');
      expect(computeExpiresAt(30, fixed)).toBe('2026-01-31T00:00:00.000Z');
    });
  });

  describe('buildSharedFileLinkHtml', () => {
    it('wraps the download link in a paragraph with size + expiry note', () => {
      const html = buildSharedFileLinkHtml(sampleSharedFile);
      expect(html).toContain('<p>');
      expect(html).toContain('big-deck.pdf');
      expect(html).toContain('/api/dl/tok-abc');
      expect(html).toContain('60.0 MB');
      expect(html).toMatch(/expires /);
      expect(html).toContain('target="_blank"');
      expect(html).toContain('rel="noopener"');
    });
    it('omits the expiry note when expires_at is null', () => {
      const html = buildSharedFileLinkHtml({ ...sampleSharedFile, expires_at: null });
      expect(html).not.toMatch(/expires/);
    });
  });
});

describe('LargeFileAttacher component', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  function makeFile(name: string, sizeBytes: number, type = 'application/pdf'): File {
    // NOTE: Build a File whose .size matches the requested byte count without
    // actually allocating that many bytes (Blob with a tiny payload, then
    // override the size getter so component logic sees the realistic size).
    const f = new File(['x'], name, { type });
    Object.defineProperty(f, 'size', { value: sizeBytes, configurable: true });
    return f;
  }

  it('renders the file picker and toggle controls', () => {
    render(<LargeFileAttacher onLinkReady={vi.fn()} />);
    expect(screen.getByTestId('large-file-input')).toBeTruthy();
    expect(screen.getByTestId('expiry-select')).toBeTruthy();
    expect(screen.getByTestId('password-input')).toBeTruthy();
    expect(screen.getByTestId('upload-and-insert-button')).toBeTruthy();
  });

  it('rejects files larger than the hard maximum', () => {
    const onError = vi.fn();
    render(<LargeFileAttacher onLinkReady={vi.fn()} onError={onError} />);
    const input = screen.getByTestId('large-file-input') as HTMLInputElement;
    const oversized = makeFile('huge.bin', MAX_SHARED_FILE_BYTES + 1);
    fireEvent.change(input, { target: { files: [oversized] } });
    expect(onError).toHaveBeenCalled();
    expect(screen.getByTestId('large-file-error').textContent).toMatch(/max allowed/);
  });

  it('uploads the file, invokes onLinkReady with link HTML, and resets state', async () => {
    mockUploadWithProgress.mockImplementation((_formData, onProgress) => {
      if (onProgress) onProgress({ loaded: 50, total: 100 });
      return Promise.resolve(sampleSharedFile);
    });
    const onLinkReady = vi.fn();
    render(<LargeFileAttacher onLinkReady={onLinkReady} />);

    const input = screen.getByTestId('large-file-input') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [makeFile('big-deck.pdf', 60 * 1024 * 1024)] } });

    fireEvent.click(screen.getByTestId('upload-and-insert-button'));

    await waitFor(() => expect(onLinkReady).toHaveBeenCalledTimes(1));
    const [html, file] = onLinkReady.mock.calls[0];
    expect(html).toContain('/api/dl/tok-abc');
    expect(file).toBe(sampleSharedFile);
  });

  it('reports network errors via onError when upload fails', async () => {
    mockUploadWithProgress.mockRejectedValue(new Error('Upload failed (500): boom'));
    const onError = vi.fn();
    render(<LargeFileAttacher onLinkReady={vi.fn()} onError={onError} />);

    const input = screen.getByTestId('large-file-input') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [makeFile('big-deck.pdf', 60 * 1024 * 1024)] } });
    fireEvent.click(screen.getByTestId('upload-and-insert-button'));

    await waitFor(() => expect(onError).toHaveBeenCalled());
    expect(onError.mock.calls[0][0]).toMatch(/Upload failed/);
  });

  it('sends expiry timestamp + password in FormData when configured', async () => {
    let capturedForm: FormData | null = null;
    mockUploadWithProgress.mockImplementation((formData: FormData) => {
      capturedForm = formData;
      return Promise.resolve(sampleSharedFile);
    });
    render(<LargeFileAttacher onLinkReady={vi.fn()} />);

    const input = screen.getByTestId('large-file-input') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [makeFile('big-deck.pdf', 60 * 1024 * 1024)] } });

    fireEvent.change(screen.getByTestId('expiry-select'), { target: { value: '7' } });
    fireEvent.change(screen.getByTestId('password-input'), { target: { value: 's3cret' } });
    fireEvent.click(screen.getByTestId('upload-and-insert-button'));

    await waitFor(() => expect(mockUploadWithProgress).toHaveBeenCalled());
    expect(capturedForm).not.toBeNull();
    expect(capturedForm!.get('password')).toBe('s3cret');
    expect(typeof capturedForm!.get('expires_at')).toBe('string');
    // NOTE: Never option produces no expires_at field at all.
  });

  it('omits expires_at when the Never option is chosen', async () => {
    let capturedForm: FormData | null = null;
    mockUploadWithProgress.mockImplementation((formData: FormData) => {
      capturedForm = formData;
      return Promise.resolve(sampleSharedFile);
    });
    render(<LargeFileAttacher onLinkReady={vi.fn()} />);

    const input = screen.getByTestId('large-file-input') as HTMLInputElement;
    fireEvent.change(input, { target: { files: [makeFile('big-deck.pdf', 60 * 1024 * 1024)] } });
    fireEvent.change(screen.getByTestId('expiry-select'), { target: { value: 'never' } });
    fireEvent.click(screen.getByTestId('upload-and-insert-button'));

    await waitFor(() => expect(mockUploadWithProgress).toHaveBeenCalled());
    expect(capturedForm!.has('expires_at')).toBe(false);
  });
});
