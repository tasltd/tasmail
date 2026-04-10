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
});
