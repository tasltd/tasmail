// Added: Unit tests for SharedMailboxManager component (TMAIL-96)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SharedMailboxManager } from './SharedMailboxManager';

// Added: Mock functions for all shared mailbox API methods
const mockListAccessible = vi.fn();
const mockListAcl = vi.fn();
const mockGrantAccess = vi.fn();
const mockRevokeAccess = vi.fn();

vi.mock('../../api/shared-mailboxes', () => ({
  sharedMailboxApi: {
    listAccessible: () => mockListAccessible(),
    listAcl: (...args: unknown[]) => mockListAcl(...args),
    grantAccess: (...args: unknown[]) => mockGrantAccess(...args),
    revokeAccess: (...args: unknown[]) => mockRevokeAccess(...args),
  },
}));

// Added: Mock mailStore to capture setViewMode calls
const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode, viewMode: 'shared' }),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('SharedMailboxManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders loading skeleton while fetching mailboxes', () => {
    // NOTE: Never-resolving promise keeps the component in loading state
    mockListAccessible.mockReturnValue(new Promise(() => {}));
    const { container } = render(<SharedMailboxManager />, { wrapper: createWrapper() });
    expect(container.querySelector('.loading-skeleton')).toBeTruthy();
  });

  it('renders the header with back button and title', async () => {
    mockListAccessible.mockResolvedValue([]);
    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared Mailboxes')).toBeInTheDocument();
    });
    expect(screen.getByText('Back')).toBeInTheDocument();
  });

  it('navigates back to list view when back button is clicked', async () => {
    mockListAccessible.mockResolvedValue([]);
    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('renders empty state when no shared mailboxes exist', async () => {
    mockListAccessible.mockResolvedValue([]);
    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/No shared mailboxes available/)).toBeInTheDocument();
    });
  });

  it('renders list of accessible shared mailboxes', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'support@example.com',
        display_name: 'Support Team',
        can_read: true,
        can_write: true,
        can_delete: false,
        can_admin: false,
      },
      {
        mailbox_id: 'mb-2',
        username: 'sales@example.com',
        display_name: null,
        can_read: true,
        can_write: false,
        can_delete: false,
        can_admin: true,
      },
    ]);
    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Support Team')).toBeInTheDocument();
      // NOTE: When display_name is null, username appears in both strong and span elements
      expect(screen.getAllByText('sales@example.com').length).toBeGreaterThanOrEqual(1);
    });
  });

  it('shows admin badge for mailboxes with admin permission', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'admin-box@example.com',
        display_name: 'Admin Box',
        can_read: true,
        can_write: true,
        can_delete: true,
        can_admin: true,
      },
    ]);
    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Admin')).toBeInTheDocument();
    });
  });

  it('expands admin mailbox to show ACL entries', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'shared@example.com',
        display_name: 'Shared Box',
        can_read: true,
        can_write: true,
        can_delete: true,
        can_admin: true,
      },
    ]);
    mockListAcl.mockResolvedValue([
      {
        id: 'acl-1',
        mailbox_id: 'mb-1',
        granted_to: 'user-1',
        granted_to_username: 'alice@example.com',
        can_read: true,
        can_write: true,
        can_delete: false,
        can_admin: false,
        granted_at: '2026-01-01T00:00:00Z',
      },
    ]);

    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared Box')).toBeInTheDocument();
    });

    // Added: Click to expand the mailbox ACL panel
    fireEvent.click(screen.getByText('Shared Box'));

    await waitFor(() => {
      expect(screen.getByText('alice@example.com')).toBeInTheDocument();
      expect(screen.getByText('Access Control')).toBeInTheDocument();
    });
  });

  it('shows grant access form when button is clicked', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'shared@example.com',
        display_name: 'Shared Box',
        can_read: true,
        can_write: true,
        can_delete: true,
        can_admin: true,
      },
    ]);
    mockListAcl.mockResolvedValue([]);

    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared Box')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Shared Box'));

    await waitFor(() => {
      expect(screen.getByText('Grant Access')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Grant Access'));

    await waitFor(() => {
      expect(screen.getByPlaceholderText('User UUID to grant access')).toBeInTheDocument();
      expect(screen.getByText('Read')).toBeInTheDocument();
      expect(screen.getByText('Write')).toBeInTheDocument();
      expect(screen.getByText('Delete')).toBeInTheDocument();
    });
  });

  it('shows empty ACL message when no entries exist', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'shared@example.com',
        display_name: 'Shared Box',
        can_read: true,
        can_write: true,
        can_delete: true,
        can_admin: true,
      },
    ]);
    mockListAcl.mockResolvedValue([]);

    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared Box')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Shared Box'));

    await waitFor(() => {
      expect(screen.getByText(/No ACL entries/)).toBeInTheDocument();
    });
  });

  it('displays user permissions for each mailbox', async () => {
    mockListAccessible.mockResolvedValue([
      {
        mailbox_id: 'mb-1',
        username: 'shared@example.com',
        display_name: 'Shared Box',
        can_read: true,
        can_write: true,
        can_delete: false,
        can_admin: false,
      },
    ]);

    render(<SharedMailboxManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Read, Write')).toBeInTheDocument();
    });
  });
});
