// Added: ArchiveManager component tests for TMAIL-107

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ArchiveManager } from './ArchiveManager';

const mockListArchivePolicies = vi.fn();
const mockCreateArchivePolicy = vi.fn();
const mockUpdateArchivePolicy = vi.fn();
const mockDeleteArchivePolicy = vi.fn();
const mockGetArchiveConfig = vi.fn();
const mockUpdateArchiveConfig = vi.fn();
const mockSearchArchive = vi.fn();
const mockGetArchiveSearchHistory = vi.fn();

vi.mock('../../api/archive', () => ({
  listArchivePolicies: () => mockListArchivePolicies(),
  createArchivePolicy: (...args: unknown[]) => mockCreateArchivePolicy(...args),
  updateArchivePolicy: (...args: unknown[]) => mockUpdateArchivePolicy(...args),
  deleteArchivePolicy: (...args: unknown[]) => mockDeleteArchivePolicy(...args),
  getArchiveConfig: () => mockGetArchiveConfig(),
  updateArchiveConfig: (...args: unknown[]) => mockUpdateArchiveConfig(...args),
  searchArchive: (...args: unknown[]) => mockSearchArchive(...args),
  getArchiveSearchHistory: () => mockGetArchiveSearchHistory(),
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

describe('ArchiveManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Email Archive heading after loading', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Archive')).toBeInTheDocument();
    });
  });

  it('shows empty state when no archive policies exist', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No archive policies configured. Add one to start archiving emails with Piler.'),
      ).toBeInTheDocument();
    });
  });

  it('renders policy list with name and status badges', async () => {
    mockListArchivePolicies.mockResolvedValue([
      {
        id: 'pol-1',
        name: 'Archive INBOX',
        description: 'Archive all inbox emails',
        match_criteria: { domains: ['*'], folders: ['INBOX'] },
        archive_after_days: 90,
        delete_original: false,
        enabled: true,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
      {
        id: 'pol-2',
        name: 'Sent Archive',
        description: null,
        match_criteria: { folders: ['Sent'] },
        archive_after_days: 365,
        delete_original: true,
        enabled: false,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Archive INBOX')).toBeInTheDocument();
      expect(screen.getByText('Sent Archive')).toBeInTheDocument();
    });
    // NOTE: Check enabled/disabled badges
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
    // NOTE: Check delete_original badge for second policy
    expect(screen.getByText('Deletes Original')).toBeInTheDocument();
  });

  it('shows add policy form when Add Policy is clicked', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Policy')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Policy'));

    expect(screen.getByText('New Archive Policy')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Archive All INBOX')).toBeInTheDocument();
  });

  it('shows domains and folders inputs in create form', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Policy')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Policy'));

    expect(screen.getByPlaceholderText('*, example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('INBOX, Sent')).toBeInTheDocument();
  });

  it('renders delete and toggle buttons for each policy', async () => {
    mockListArchivePolicies.mockResolvedValue([
      {
        id: 'pol-1',
        name: 'Policy A',
        description: null,
        match_criteria: {},
        archive_after_days: 30,
        delete_original: false,
        enabled: true,
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(1);
    });
    expect(screen.getByTestId('toggle-pol-1')).toBeInTheDocument();
  });

  it('switches to config tab and shows config form', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    mockGetArchiveConfig.mockResolvedValue(null);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Archive')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Config'));

    await waitFor(() => {
      expect(screen.getByTestId('archive-config-panel')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('https://piler.example.com')).toBeInTheDocument();
      expect(screen.getByText('Save Configuration')).toBeInTheDocument();
    });
  });

  it('switches to search tab and shows search form', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    mockGetArchiveSearchHistory.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Archive')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      expect(screen.getByTestId('archive-search-panel')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('Search archived emails...')).toBeInTheDocument();
      expect(screen.getByText('Search Archive')).toBeInTheDocument();
    });
  });

  it('navigates back to list view when back button is clicked', async () => {
    mockListArchivePolicies.mockResolvedValue([]);
    render(<ArchiveManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
