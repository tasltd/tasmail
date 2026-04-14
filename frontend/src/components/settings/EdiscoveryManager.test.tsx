// Added: EdiscoveryManager component tests for TMAIL-137

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EdiscoveryManager } from './EdiscoveryManager';

const mockListEdiscoverySearches = vi.fn();
const mockCreateEdiscoverySearch = vi.fn();
const mockGetEdiscoverySearch = vi.fn();
const mockDeleteEdiscoverySearch = vi.fn();
const mockExecuteEdiscoverySearch = vi.fn();
const mockExportEdiscoveryResults = vi.fn();

vi.mock('../../api/ediscovery', () => ({
  listEdiscoverySearches: () => mockListEdiscoverySearches(),
  createEdiscoverySearch: (...args: unknown[]) => mockCreateEdiscoverySearch(...args),
  getEdiscoverySearch: (...args: unknown[]) => mockGetEdiscoverySearch(...args),
  deleteEdiscoverySearch: (...args: unknown[]) => mockDeleteEdiscoverySearch(...args),
  executeEdiscoverySearch: (...args: unknown[]) => mockExecuteEdiscoverySearch(...args),
  exportEdiscoveryResults: (...args: unknown[]) => mockExportEdiscoveryResults(...args),
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

describe('EdiscoveryManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders eDiscovery heading after loading', async () => {
    mockListEdiscoverySearches.mockResolvedValue([]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('eDiscovery')).toBeInTheDocument();
    });
  });

  it('shows empty state when no searches exist', async () => {
    mockListEdiscoverySearches.mockResolvedValue([]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No eDiscovery searches yet. Create one to search across user mailboxes.'),
      ).toBeInTheDocument();
    });
  });

  it('renders search list with name and status', async () => {
    mockListEdiscoverySearches.mockResolvedValue([
      {
        id: 'ed-1',
        admin_id: 'admin-1',
        name: 'Q1 Compliance Review',
        description: null,
        search_query: 'quarterly report',
        target_users: null,
        date_from: null,
        date_to: null,
        include_attachments: false,
        status: 'Pending',
        results_count: 0,
        export_path: null,
        created_at: '2026-04-01T00:00:00Z',
        completed_at: null,
      },
      {
        id: 'ed-2',
        admin_id: 'admin-1',
        name: 'Legal Investigation',
        description: 'Contract breach case',
        search_query: 'breach',
        target_users: ['user-1'],
        date_from: '2026-01-01T00:00:00Z',
        date_to: '2026-03-31T00:00:00Z',
        include_attachments: true,
        status: 'Completed',
        results_count: 15,
        export_path: null,
        created_at: '2026-04-02T00:00:00Z',
        completed_at: '2026-04-02T01:00:00Z',
      },
    ]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Q1 Compliance Review')).toBeInTheDocument();
      expect(screen.getByText('Legal Investigation')).toBeInTheDocument();
    });
    // NOTE: Check status badges are rendered
    expect(screen.getByText('Pending')).toBeInTheDocument();
    expect(screen.getByText('Completed')).toBeInTheDocument();
  });

  it('shows create form when New Search is clicked', async () => {
    mockListEdiscoverySearches.mockResolvedValue([]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Search')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Search'));

    expect(screen.getByText('New eDiscovery Search')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g., Q1 Compliance Review')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g., confidential contract')).toBeInTheDocument();
  });

  it('shows execute button for pending searches', async () => {
    mockListEdiscoverySearches.mockResolvedValue([
      {
        id: 'ed-pending',
        admin_id: 'admin-1',
        name: 'Pending Search',
        description: null,
        search_query: 'test',
        target_users: null,
        date_from: null,
        date_to: null,
        include_attachments: false,
        status: 'Pending',
        results_count: 0,
        export_path: null,
        created_at: '2026-04-01T00:00:00Z',
        completed_at: null,
      },
    ]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('execute-ed-pending')).toBeInTheDocument();
    });
  });

  it('shows export button for completed searches', async () => {
    mockListEdiscoverySearches.mockResolvedValue([
      {
        id: 'ed-done',
        admin_id: 'admin-1',
        name: 'Done Search',
        description: null,
        search_query: 'test',
        target_users: null,
        date_from: null,
        date_to: null,
        include_attachments: false,
        status: 'Completed',
        results_count: 10,
        export_path: null,
        created_at: '2026-04-01T00:00:00Z',
        completed_at: '2026-04-01T01:00:00Z',
      },
    ]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('export-ed-done')).toBeInTheDocument();
    });
  });

  it('shows view details button for each search', async () => {
    mockListEdiscoverySearches.mockResolvedValue([
      {
        id: 'ed-1',
        admin_id: 'admin-1',
        name: 'Search One',
        description: null,
        search_query: 'keyword',
        target_users: null,
        date_from: null,
        date_to: null,
        include_attachments: false,
        status: 'Completed',
        results_count: 5,
        export_path: null,
        created_at: '2026-04-01T00:00:00Z',
        completed_at: '2026-04-01T01:00:00Z',
      },
    ]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('view-ed-1')).toBeInTheDocument();
    });
  });

  it('navigates back to list when back button is clicked', async () => {
    mockListEdiscoverySearches.mockResolvedValue([]);
    render(<EdiscoveryManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
