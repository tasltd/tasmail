// Added: RetentionManager component tests for TMAIL-109

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RetentionManager } from './RetentionManager';

const mockListRetentionPolicies = vi.fn();
const mockCreateRetentionPolicy = vi.fn();
const mockUpdateRetentionPolicy = vi.fn();
const mockDeleteRetentionPolicy = vi.fn();
const mockListLegalHolds = vi.fn();
const mockCreateLegalHold = vi.fn();
const mockReleaseLegalHold = vi.fn();

vi.mock('../../api/retention', () => ({
  listRetentionPolicies: () => mockListRetentionPolicies(),
  createRetentionPolicy: (...args: unknown[]) => mockCreateRetentionPolicy(...args),
  updateRetentionPolicy: (...args: unknown[]) => mockUpdateRetentionPolicy(...args),
  deleteRetentionPolicy: (...args: unknown[]) => mockDeleteRetentionPolicy(...args),
  listLegalHolds: () => mockListLegalHolds(),
  createLegalHold: (...args: unknown[]) => mockCreateLegalHold(...args),
  releaseLegalHold: (...args: unknown[]) => mockReleaseLegalHold(...args),
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

describe('RetentionManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Retention & Legal Hold heading after loading', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Retention & Legal Hold')).toBeInTheDocument();
    });
  });

  it('shows empty state for policies when none exist', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No retention policies configured. Add one to set auto-deletion rules.'),
      ).toBeInTheDocument();
    });
  });

  it('renders policy list with name and retention days', async () => {
    mockListRetentionPolicies.mockResolvedValue([
      {
        id: 'rp-1',
        name: 'Trash Cleanup',
        description: 'Delete trash emails',
        retention_days: 30,
        folder_pattern: 'Trash',
        apply_to_all: true,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
      {
        id: 'rp-2',
        name: 'Spam Removal',
        description: null,
        retention_days: 7,
        folder_pattern: 'Spam',
        apply_to_all: false,
        created_at: '2026-04-02T00:00:00Z',
        updated_at: '2026-04-02T00:00:00Z',
      },
    ]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Trash Cleanup')).toBeInTheDocument();
      expect(screen.getByText('Spam Removal')).toBeInTheDocument();
    });
    // NOTE: Check retention days are displayed
    expect(screen.getByText(/30 days/)).toBeInTheDocument();
    expect(screen.getByText(/7 days/)).toBeInTheDocument();
  });

  it('shows add policy form when Add Policy is clicked', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Policy')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Policy'));

    expect(screen.getByText('New Retention Policy')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g., Trash Cleanup')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g., 30')).toBeInTheDocument();
  });

  it('renders Legal Holds section heading', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Legal Holds')).toBeInTheDocument();
    });
  });

  it('shows active holds with user and reason', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([
      {
        id: 'lh-1',
        user_id: 'abcdef12-3456-7890-abcd-ef1234567890',
        reason: 'Ongoing litigation',
        placed_by: 'admin-001',
        active: true,
        created_at: '2026-04-05T00:00:00Z',
        released_at: null,
      },
      {
        id: 'lh-2',
        user_id: '12345678-abcd-ef01-2345-678901234567',
        reason: 'Compliance audit',
        placed_by: 'admin-002',
        active: false,
        created_at: '2026-03-01T00:00:00Z',
        released_at: '2026-04-01T00:00:00Z',
      },
    ]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      // NOTE: Reason text is inside a div with other text, so use substring matching
      expect(screen.getByText(/Ongoing litigation/)).toBeInTheDocument();
      expect(screen.getByText(/Compliance audit/)).toBeInTheDocument();
    });
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Released')).toBeInTheDocument();
  });

  it('shows place hold form when Place Hold is clicked', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Place Hold')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Place Hold'));

    expect(screen.getByText('Place Legal Hold')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('User UUID')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Ongoing litigation/)).toBeInTheDocument();
  });

  it('shows release button for active holds only', async () => {
    mockListRetentionPolicies.mockResolvedValue([]);
    mockListLegalHolds.mockResolvedValue([
      {
        id: 'lh-1',
        user_id: 'abcdef12-3456-7890-abcd-ef1234567890',
        reason: 'Active hold',
        placed_by: 'admin-001',
        active: true,
        created_at: '2026-04-05T00:00:00Z',
        released_at: null,
      },
      {
        id: 'lh-2',
        user_id: '12345678-abcd-ef01-2345-678901234567',
        reason: 'Released hold',
        placed_by: 'admin-002',
        active: false,
        created_at: '2026-03-01T00:00:00Z',
        released_at: '2026-04-01T00:00:00Z',
      },
    ]);
    render(<RetentionManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      // NOTE: Only one release button should exist (for the active hold)
      expect(screen.getByTestId('release-lh-1')).toBeInTheDocument();
    });
    // NOTE: Released hold should not have a release button
    expect(screen.queryByTestId('release-lh-2')).not.toBeInTheDocument();
  });
});
