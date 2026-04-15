// Added: Unit tests for AttachmentManager component (TMAIL-59)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AttachmentManager } from './AttachmentManager';

const mockList = vi.fn();
const mockUpload = vi.fn();
const mockDownload = vi.fn();
const mockDelete = vi.fn();
const mockStats = vi.fn();

vi.mock('../../api/attachments', () => ({
  attachmentsApi: {
    list: () => mockList(),
    upload: (...args: unknown[]) => mockUpload(...args),
    download: (...args: unknown[]) => mockDownload(...args),
    delete: (...args: unknown[]) => mockDelete(...args),
    stats: () => mockStats(),
  },
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

// Added: Sample attachment fixtures for testing
const sampleAttachments = [
  {
    id: 'att-1',
    mailbox_id: 'mb-1',
    message_uid: 42,
    folder: 'INBOX',
    filename: 'report.pdf',
    content_type: 'application/pdf',
    size_bytes: 1048576,
    storage_path: '/data/att/report.pdf',
    checksum: 'abc123',
    scan_status: 'clean' as const,
    scan_result: null,
    scanned_at: '2026-04-14T10:00:00Z',
    created_at: '2026-04-14T09:00:00Z',
  },
  {
    id: 'att-2',
    mailbox_id: 'mb-1',
    message_uid: null,
    folder: null,
    filename: 'photo.jpg',
    content_type: 'image/jpeg',
    size_bytes: 524288,
    storage_path: '/data/att/photo.jpg',
    checksum: 'def456',
    scan_status: 'pending' as const,
    scan_result: null,
    scanned_at: null,
    created_at: '2026-04-14T11:00:00Z',
  },
];

const sampleStats = {
  total_count: 2,
  total_size_bytes: 1572864,
  pending_scans: 1,
  infected_count: 0,
};

describe('AttachmentManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and upload form after loading', async () => {
    mockList.mockResolvedValue([]);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Attachment Storage')).toBeInTheDocument();
    });
    expect(screen.getByText('Upload Attachment')).toBeInTheDocument();
  });

  it('shows empty state when no attachments exist', async () => {
    mockList.mockResolvedValue([]);
    mockStats.mockResolvedValue({ total_count: 0, total_size_bytes: 0, pending_scans: 0, infected_count: 0 });
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No attachments yet. Upload a file to get started.'),
      ).toBeInTheDocument();
    });
  });

  it('renders attachment list with filenames and scan status', async () => {
    mockList.mockResolvedValue(sampleAttachments);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('report.pdf')).toBeInTheDocument();
      expect(screen.getByText('photo.jpg')).toBeInTheDocument();
    });
    expect(screen.getByText('clean')).toBeInTheDocument();
    expect(screen.getByText('pending')).toBeInTheDocument();
  });

  it('renders storage stats when available', async () => {
    mockList.mockResolvedValue(sampleAttachments);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('storage-stats')).toBeInTheDocument();
    });
    // NOTE: 1572864 bytes = 1.5 MB
    expect(screen.getByText('1.5 MB')).toBeInTheDocument();
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('renders file input for upload', async () => {
    mockList.mockResolvedValue([]);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('file-input')).toBeInTheDocument();
    });
  });

  it('renders download and delete buttons for each attachment', async () => {
    mockList.mockResolvedValue(sampleAttachments);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('download-att-1')).toBeInTheDocument();
      expect(screen.getByTestId('download-att-2')).toBeInTheDocument();
      expect(screen.getByTestId('delete-att-1')).toBeInTheDocument();
      expect(screen.getByTestId('delete-att-2')).toBeInTheDocument();
    });
  });

  it('shows file size and content type in attachment details', async () => {
    mockList.mockResolvedValue(sampleAttachments);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/1\.0 MB/)).toBeInTheDocument();
      expect(screen.getByText(/application\/pdf/)).toBeInTheDocument();
      expect(screen.getByText(/image\/jpeg/)).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', async () => {
    mockList.mockResolvedValue([]);
    mockStats.mockResolvedValue(sampleStats);
    render(<AttachmentManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
