// Added: LdapManager component tests for TMAIL-100
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { LdapManager } from './LdapManager';

const mockListLdapConfigs = vi.fn();
const mockCreateLdapConfig = vi.fn();
const mockUpdateLdapConfig = vi.fn();
const mockDeleteLdapConfig = vi.fn();
const mockTriggerLdapSync = vi.fn();
const mockListLdapSyncLogs = vi.fn();

vi.mock('../../api/ldap', () => ({
  listLdapConfigs: () => mockListLdapConfigs(),
  createLdapConfig: (...args: unknown[]) => mockCreateLdapConfig(...args),
  updateLdapConfig: (...args: unknown[]) => mockUpdateLdapConfig(...args),
  deleteLdapConfig: (...args: unknown[]) => mockDeleteLdapConfig(...args),
  triggerLdapSync: (...args: unknown[]) => mockTriggerLdapSync(...args),
  listLdapSyncLogs: (...args: unknown[]) => mockListLdapSyncLogs(...args),
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

const mockConfig = {
  id: 'cfg-1',
  name: 'Corporate AD',
  server_url: 'ldaps://ad.example.com:636',
  bind_dn: 'cn=admin,dc=example,dc=com',
  search_base: 'ou=Users,dc=example,dc=com',
  search_filter: '(objectClass=person)',
  email_attribute: 'mail',
  name_attribute: 'displayName',
  group_filter: null,
  sync_interval_minutes: 60,
  active: true,
  last_sync_at: '2026-04-10T12:00:00Z',
  last_sync_status: 'completed',
  users_synced: 42,
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-10T12:00:00Z',
};

describe('LdapManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading', async () => {
    mockListLdapConfigs.mockResolvedValue([]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('LDAP / Active Directory')).toBeInTheDocument();
    });
  });

  it('shows empty state when no configs exist', async () => {
    mockListLdapConfigs.mockResolvedValue([]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No LDAP configurations yet.')).toBeInTheDocument();
    });
  });

  it('displays config list with name and server URL', async () => {
    mockListLdapConfigs.mockResolvedValue([mockConfig]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Corporate AD')).toBeInTheDocument();
    });
    expect(screen.getByText('ldaps://ad.example.com:636')).toBeInTheDocument();
    expect(screen.getByText('42 users')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('shows add form when Add Configuration is clicked', async () => {
    mockListLdapConfigs.mockResolvedValue([]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Configuration')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Configuration'));

    expect(screen.getByLabelText('Configuration Name')).toBeInTheDocument();
    expect(screen.getByLabelText('Server URL')).toBeInTheDocument();
    expect(screen.getByLabelText('Bind DN')).toBeInTheDocument();
    expect(screen.getByLabelText('Bind Password')).toBeInTheDocument();
    expect(screen.getByLabelText('Search Base')).toBeInTheDocument();
  });

  it('shows attribute mapping fields in the form', async () => {
    mockListLdapConfigs.mockResolvedValue([]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Configuration')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Configuration'));

    expect(screen.getByLabelText('Email Attribute')).toBeInTheDocument();
    expect(screen.getByLabelText('Email Attribute')).toHaveValue('mail');
    expect(screen.getByLabelText('Name Attribute')).toBeInTheDocument();
    expect(screen.getByLabelText('Name Attribute')).toHaveValue('displayName');
    expect(screen.getByLabelText('Search Filter')).toHaveValue('(objectClass=person)');
    expect(screen.getByLabelText('Sync Interval (minutes)')).toHaveValue(60);
  });

  it('shows sync now button for each config', async () => {
    mockListLdapConfigs.mockResolvedValue([mockConfig]);
    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Sync now')).toBeInTheDocument();
    });
  });

  it('shows sync history toggle for each config', async () => {
    mockListLdapConfigs.mockResolvedValue([mockConfig]);
    mockListLdapSyncLogs.mockResolvedValue([
      {
        id: 'log-1',
        config_id: 'cfg-1',
        started_at: '2026-04-10T12:00:00Z',
        completed_at: '2026-04-10T12:01:00Z',
        users_created: 5,
        users_updated: 10,
        users_disabled: 2,
        errors: [],
        status: 'completed',
      },
    ]);

    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Sync History')).toBeInTheDocument();
    });

    // Added: Click to expand sync history
    fireEvent.click(screen.getByText('Sync History'));

    await waitFor(() => {
      expect(screen.getByText('completed')).toBeInTheDocument();
    });
  });

  it('shows error details in sync log when errors exist', async () => {
    mockListLdapConfigs.mockResolvedValue([mockConfig]);
    mockListLdapSyncLogs.mockResolvedValue([
      {
        id: 'log-2',
        config_id: 'cfg-1',
        started_at: '2026-04-10T12:00:00Z',
        completed_at: '2026-04-10T12:01:00Z',
        users_created: 3,
        users_updated: 0,
        users_disabled: 0,
        errors: [
          { email: 'bad@example.com', error: 'duplicate email' },
          { email: 'invalid@test.com', error: 'missing name' },
        ],
        status: 'completed',
      },
    ]);

    render(<LdapManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Sync History')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Sync History'));

    await waitFor(() => {
      expect(screen.getByText('2 errors')).toBeInTheDocument();
    });
  });
});
