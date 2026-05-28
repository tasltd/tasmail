import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MigrationManager } from './MigrationManager';
import type { MigrationJob } from '../../types/migration';

const mockList = vi.fn();
const mockStartImap = vi.fn();
const mockStartMbox = vi.fn();
const mockCancel = vi.fn();

vi.mock('../../api/migration', () => ({
  migrationApi: {
    list: (...args: unknown[]) => mockList(...args),
    startImap: (...args: unknown[]) => mockStartImap(...args),
    startMbox: (...args: unknown[]) => mockStartMbox(...args),
    cancel: (...args: unknown[]) => mockCancel(...args),
  },
}));

// Added: Mocks for the MBOX folder export flow (TMAIL-68)
const mockFetchFolders = vi.fn();
const mockExportFolderMbox = vi.fn();
const mockDownloadMbox = vi.fn();

vi.mock('../../api/folders', () => ({
  fetchFolders: (...args: unknown[]) => mockFetchFolders(...args),
}));

vi.mock('../../api/eml', () => ({
  exportFolderMbox: (...args: unknown[]) => mockExportFolderMbox(...args),
  downloadMbox: (...args: unknown[]) => mockDownloadMbox(...args),
}));

function createWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe('MigrationManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetchFolders.mockResolvedValue([
      { name: 'INBOX', total: 0, unseen: 0 },
      { name: 'Sent', total: 0, unseen: 0 },
      { name: 'Archive', total: 0, unseen: 0 },
    ]);
  });

  it('renders "Email Migration" heading', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Migration')).toBeInTheDocument();
    });
  });

  it('shows IMAP and MBOX tab buttons', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('IMAP Migration')).toBeInTheDocument();
    });
    expect(screen.getByText('MBOX Import')).toBeInTheDocument();
  });

  it('shows IMAP form fields (server, port, username, password, SSL checkbox)', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByPlaceholderText('imap.gmail.com')).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText('993')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('user@gmail.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('App-specific password')).toBeInTheDocument();
    expect(screen.getByLabelText('Use SSL/TLS')).toBeInTheDocument();
  });

  it('shows MBOX form with file path input when MBOX tab clicked', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MBOX Import')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('MBOX Import'));

    await waitFor(() => {
      expect(screen.getByPlaceholderText('/path/to/takeout.mbox')).toBeInTheDocument();
    });
    expect(screen.getByText('MBOX File Path')).toBeInTheDocument();
  });

  it('shows migration job card with progress', async () => {
    const jobs: MigrationJob[] = [
      {
        id: 'job-1',
        job_type: 'imap',
        status: 'running',
        source_host: 'imap.gmail.com',
        messages_total: 100,
        messages_done: 45,
        error_message: null,
      } as MigrationJob,
    ];
    mockList.mockResolvedValue(jobs);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('IMAP')).toBeInTheDocument();
    });
    expect(screen.getByText('imap.gmail.com')).toBeInTheDocument();
    expect(screen.getByText('running')).toBeInTheDocument();
    expect(screen.getByText('45/100 messages (45%)')).toBeInTheDocument();
  });

  it('shows cancel button on active jobs', async () => {
    const jobs: MigrationJob[] = [
      {
        id: 'job-2',
        job_type: 'imap',
        status: 'running',
        source_host: 'imap.outlook.com',
        messages_total: 50,
        messages_done: 10,
        error_message: null,
      } as MigrationJob,
    ];
    mockList.mockResolvedValue(jobs);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Cancel')).toBeInTheDocument();
    });
  });

  // Added: Tests for MBOX folder export tab (TMAIL-68)
  it('shows MBOX Export tab button', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MBOX Export')).toBeInTheDocument();
    });
  });

  it('shows folder dropdown when MBOX Export tab is clicked', async () => {
    mockList.mockResolvedValue([]);
    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MBOX Export')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('MBOX Export'));

    await waitFor(() => {
      expect(screen.getByLabelText('Folder')).toBeInTheDocument();
    });
    // NOTE: Scope option queries to the export dropdown — the PST Import section
    //       has its own folder dropdown with overlapping option names.
    const dropdown = screen.getByLabelText('Folder') as HTMLSelectElement;
    await waitFor(() => {
      const options = Array.from(dropdown.querySelectorAll('option')).map((o) => o.textContent);
      expect(options).toContain('INBOX');
      expect(options).toContain('Sent');
      expect(options).toContain('Archive');
    });
    expect(screen.getByText('Download .mbox')).toBeInTheDocument();
  });

  it('triggers exportFolderMbox + downloadMbox when Download .mbox is clicked', async () => {
    mockList.mockResolvedValue([]);
    const fakeBlob = new Blob(['mbox-bytes'], { type: 'application/mbox' });
    mockExportFolderMbox.mockResolvedValue(fakeBlob);

    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MBOX Export')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('MBOX Export'));

    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'Sent' })).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText('Folder'), { target: { value: 'Sent' } });
    fireEvent.click(screen.getByText('Download .mbox'));

    await waitFor(() => {
      expect(mockExportFolderMbox).toHaveBeenCalledWith('Sent');
    });
    await waitFor(() => {
      expect(mockDownloadMbox).toHaveBeenCalledWith(fakeBlob, 'Sent');
    });
  });

  it('shows an error message when export fails', async () => {
    mockList.mockResolvedValue([]);
    mockExportFolderMbox.mockRejectedValue(
      new Error("MBOX export failed for folder 'INBOX': 502 — IMAP unreachable"),
    );

    render(<MigrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MBOX Export')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('MBOX Export'));

    await waitFor(() => {
      expect(screen.getByLabelText('Folder')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('Download .mbox'));

    await waitFor(() => {
      expect(screen.getByText(/MBOX export failed/)).toBeInTheDocument();
    });
    expect(mockDownloadMbox).not.toHaveBeenCalled();
  });
});
