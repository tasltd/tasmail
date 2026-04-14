// Added: Unit tests for QueueManager component (TMAIL-58)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { QueueManager } from './QueueManager';

const mockFetchQueueItems = vi.fn();
const mockFetchQueueStats = vi.fn();
const mockCancelQueueItem = vi.fn();
const mockRetryQueueItem = vi.fn();

vi.mock('../../api/queue', () => ({
  fetchQueueItems: (...args: unknown[]) => mockFetchQueueItems(...args),
  fetchQueueStats: () => mockFetchQueueStats(),
  cancelQueueItem: (...args: unknown[]) => mockCancelQueueItem(...args),
  retryQueueItem: (...args: unknown[]) => mockRetryQueueItem(...args),
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

describe('QueueManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton while fetching', () => {
    mockFetchQueueItems.mockReturnValue(new Promise(() => {}));
    mockFetchQueueStats.mockReturnValue(new Promise(() => {}));
    render(<QueueManager />, { wrapper: createWrapper() });
    // NOTE: LoadingSkeleton renders placeholder divs, header not yet visible
    expect(screen.queryByText('Email Queue')).not.toBeInTheDocument();
  });

  it('renders header and refresh button', async () => {
    mockFetchQueueItems.mockResolvedValue([]);
    mockFetchQueueStats.mockResolvedValue({ pending: 0, sending: 0, sent: 0, failed: 0, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Queue')).toBeInTheDocument();
    });
    expect(screen.getByText('Refresh')).toBeInTheDocument();
  });

  it('renders empty state when no queue items', async () => {
    mockFetchQueueItems.mockResolvedValue([]);
    mockFetchQueueStats.mockResolvedValue({ pending: 0, sending: 0, sent: 0, failed: 0, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No queued emails.')).toBeInTheDocument();
    });
  });

  it('renders queue stats bar', async () => {
    mockFetchQueueItems.mockResolvedValue([]);
    mockFetchQueueStats.mockResolvedValue({ pending: 3, sending: 1, sent: 50, failed: 2, dead_letter: 1 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('3')).toBeInTheDocument();
      expect(screen.getByText('50')).toBeInTheDocument();
    });
  });

  it('renders queue items with subject and status', async () => {
    mockFetchQueueItems.mockResolvedValue([
      {
        id: '1',
        subject: 'Test Email',
        to_addresses: ['user@test.com'],
        status: 'pending',
        retry_count: 0,
        max_retries: 5,
        last_error: null,
        created_at: '2026-04-14T10:00:00Z',
      },
      {
        id: '2',
        subject: 'Failed Email',
        to_addresses: ['fail@test.com'],
        status: 'failed',
        retry_count: 2,
        max_retries: 5,
        last_error: 'Connection refused',
        created_at: '2026-04-14T09:00:00Z',
      },
    ]);
    mockFetchQueueStats.mockResolvedValue({ pending: 1, sending: 0, sent: 0, failed: 1, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Test Email')).toBeInTheDocument();
      expect(screen.getByText('Failed Email')).toBeInTheDocument();
    });
    // NOTE: 'Pending' and 'Failed' appear in multiple places (stats bar, filter buttons, status badges)
    // so we use getAllByText to verify they exist at least once
    expect(screen.getAllByText('Pending').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Failed').length).toBeGreaterThanOrEqual(1);
    // NOTE: Error text is prefixed with "Error: " in the component
    expect(screen.getByText(/Connection refused/)).toBeInTheDocument();
  });

  it('navigates back when back button is clicked', async () => {
    mockFetchQueueItems.mockResolvedValue([]);
    mockFetchQueueStats.mockResolvedValue({ pending: 0, sending: 0, sent: 0, failed: 0, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('renders retry button for failed items', async () => {
    mockFetchQueueItems.mockResolvedValue([
      {
        id: '1',
        subject: 'Failed Email',
        to_addresses: ['user@test.com'],
        status: 'failed',
        retry_count: 1,
        max_retries: 5,
        last_error: 'Timeout',
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);
    mockFetchQueueStats.mockResolvedValue({ pending: 0, sending: 0, sent: 0, failed: 1, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Retry')).toBeInTheDocument();
    });
  });

  it('renders filter buttons including All and Dead Letter', async () => {
    mockFetchQueueItems.mockResolvedValue([]);
    mockFetchQueueStats.mockResolvedValue({ pending: 0, sending: 0, sent: 0, failed: 0, dead_letter: 0 });
    render(<QueueManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('All')).toBeInTheDocument();
      expect(screen.getByText('Dead Letter')).toBeInTheDocument();
    });
  });
});
