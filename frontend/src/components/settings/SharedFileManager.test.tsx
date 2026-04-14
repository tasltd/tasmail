// Added: Unit tests for SharedFileManager component (TMAIL-138)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SharedFileManager } from './SharedFileManager';

const mockListSharedFiles = vi.fn();
const mockUploadSharedFile = vi.fn();
const mockDeleteSharedFile = vi.fn();
const mockGetDownloadUrl = vi.fn();

vi.mock('../../api/shared-files', () => ({
  listSharedFiles: () => mockListSharedFiles(),
  uploadSharedFile: (...args: unknown[]) => mockUploadSharedFile(...args),
  deleteSharedFile: (...args: unknown[]) => mockDeleteSharedFile(...args),
  getDownloadUrl: (token: string) => mockGetDownloadUrl(token) || `http://localhost/api/dl/${token}`,
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

// Added: Sample shared file fixtures for testing
const sampleFiles = [
  {
    id: 'file-1',
    user_id: 'user-1',
    filename: 'presentation.pdf',
    content_type: 'application/pdf',
    file_size: 5242880,
    storage_path: '/data/shared/presentation.pdf',
    download_token: 'token-abc',
    download_count: 3,
    max_downloads: 10,
    expires_at: '2027-12-31T23:59:59Z',
    password_hash: null,
    created_at: '2026-04-14T10:00:00Z',
  },
  {
    id: 'file-2',
    user_id: 'user-1',
    filename: 'report.docx',
    content_type: 'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    file_size: 102400,
    storage_path: '/data/shared/report.docx',
    download_token: 'token-def',
    download_count: 5,
    max_downloads: 5,
    expires_at: null,
    password_hash: null,
    created_at: '2026-04-13T08:00:00Z',
  },
];

describe('SharedFileManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and upload form', async () => {
    mockListSharedFiles.mockResolvedValue([]);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared Files')).toBeInTheDocument();
    });
    expect(screen.getByText('Upload File')).toBeInTheDocument();
  });

  it('shows empty state when no files exist', async () => {
    mockListSharedFiles.mockResolvedValue([]);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No shared files yet. Upload a file to generate a shareable link.')).toBeInTheDocument();
    });
  });

  it('renders file list with filenames and download counts', async () => {
    mockListSharedFiles.mockResolvedValue(sampleFiles);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('presentation.pdf')).toBeInTheDocument();
      expect(screen.getByText('report.docx')).toBeInTheDocument();
    });
    // NOTE: Check download count display
    expect(screen.getByText(/3 downloads/)).toBeInTheDocument();
    expect(screen.getByText(/5 downloads/)).toBeInTheDocument();
  });

  it('renders upload form with file input', async () => {
    mockListSharedFiles.mockResolvedValue([]);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Upload & Share')).toBeInTheDocument();
    });
    expect(screen.getByTestId('file-input')).toBeInTheDocument();
  });

  it('renders expiry date input', async () => {
    mockListSharedFiles.mockResolvedValue([]);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('expiry-input')).toBeInTheDocument();
    });
  });

  it('renders max downloads input', async () => {
    mockListSharedFiles.mockResolvedValue([]);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('max-downloads-input')).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText('Unlimited')).toBeInTheDocument();
  });

  it('renders copy link button for each file', async () => {
    mockListSharedFiles.mockResolvedValue(sampleFiles);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('copy-link-file-1')).toBeInTheDocument();
      expect(screen.getByTestId('copy-link-file-2')).toBeInTheDocument();
    });
  });

  it('renders delete button for each file', async () => {
    mockListSharedFiles.mockResolvedValue(sampleFiles);
    render(<SharedFileManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('delete-file-1')).toBeInTheDocument();
      expect(screen.getByTestId('delete-file-2')).toBeInTheDocument();
    });
  });
});
