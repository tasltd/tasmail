// Added: DavConfigManager component tests for TMAIL-117

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DavConfigManager } from './DavConfigManager';

const mockListDavConfigs = vi.fn();
const mockCreateDavConfig = vi.fn();
const mockUpdateDavConfig = vi.fn();
const mockDeleteDavConfig = vi.fn();
const mockSyncDavConfig = vi.fn();
const mockTestDavConfig = vi.fn();

vi.mock('../../api/dav-config', () => ({
  listDavConfigs: () => mockListDavConfigs(),
  createDavConfig: (...args: unknown[]) => mockCreateDavConfig(...args),
  updateDavConfig: (...args: unknown[]) => mockUpdateDavConfig(...args),
  deleteDavConfig: (...args: unknown[]) => mockDeleteDavConfig(...args),
  syncDavConfig: (...args: unknown[]) => mockSyncDavConfig(...args),
  testDavConfig: (...args: unknown[]) => mockTestDavConfig(...args),
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

describe('DavConfigManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders CalDAV / CardDAV heading after loading', async () => {
    mockListDavConfigs.mockResolvedValue([]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('CalDAV / CardDAV')).toBeInTheDocument();
    });
  });

  it('shows empty state when no configs exist', async () => {
    mockListDavConfigs.mockResolvedValue([]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No CalDAV/CardDAV servers configured. Add one to sync calendars and contacts.'),
      ).toBeInTheDocument();
    });
  });

  it('shows create form with type dropdown when adding', async () => {
    mockListDavConfigs.mockResolvedValue([]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add DAV Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add DAV Server'));

    const typeSelect = screen.getByTestId('dav-type-select');
    expect(typeSelect).toBeInTheDocument();
    const options = typeSelect.querySelectorAll('option');
    expect(options.length).toBe(3);
    expect(options[0].textContent).toBe('CalDAV (Calendars)');
    expect(options[1].textContent).toBe('CardDAV (Contacts)');
    expect(options[2].textContent).toBe('Both (Calendars + Contacts)');
  });

  it('shows password input as password type', async () => {
    mockListDavConfigs.mockResolvedValue([]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add DAV Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add DAV Server'));

    const passwordInput = screen.getByTestId('password-input') as HTMLInputElement;
    expect(passwordInput).toBeInTheDocument();
    expect(passwordInput.type).toBe('password');
  });

  it('shows preset buttons in create form', async () => {
    mockListDavConfigs.mockResolvedValue([]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add DAV Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add DAV Server'));

    expect(screen.getByTestId('preset-radicale')).toBeInTheDocument();
    expect(screen.getByTestId('preset-nextcloud')).toBeInTheDocument();
    expect(screen.getByTestId('preset-icloud')).toBeInTheDocument();
    expect(screen.getByTestId('preset-google')).toBeInTheDocument();
  });

  it('shows test, sync, and delete buttons for each configuration', async () => {
    mockListDavConfigs.mockResolvedValue([
      {
        id: 'dav-1',
        name: 'Radicale',
        server_url: 'https://radicale.example.com',
        username: 'user@example.com',
        password_masked: 'my...rd',
        dav_type: 'both',
        sync_interval_minutes: 60,
        last_sync_at: '2026-04-14T00:00:00Z',
        sync_status: 'idle',
        sync_error: null,
        enabled: true,
      },
    ]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('test-dav-1')).toBeInTheDocument();
    });
    expect(screen.getByTestId('sync-dav-1')).toBeInTheDocument();
    expect(screen.getByTestId('delete-dav-1')).toBeInTheDocument();
  });

  it('shows type badge and enabled status for configs', async () => {
    mockListDavConfigs.mockResolvedValue([
      {
        id: 'dav-1',
        name: 'Nextcloud',
        server_url: 'https://cloud.example.com/remote.php/dav',
        username: 'user',
        password_masked: 'pa...rd',
        dav_type: 'both',
        sync_interval_minutes: 30,
        last_sync_at: null,
        sync_status: 'idle',
        sync_error: null,
        enabled: true,
      },
    ]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Nextcloud')).toBeInTheDocument();
    });
    expect(screen.getByTestId('type-badge')).toBeInTheDocument();
    expect(screen.getByText('both')).toBeInTheDocument();
    expect(screen.getByText('Enabled')).toBeInTheDocument();
  });

  it('renders config list with server url and sync interval', async () => {
    mockListDavConfigs.mockResolvedValue([
      {
        id: 'dav-1',
        name: 'Radicale',
        server_url: 'https://radicale.example.com',
        username: 'user@example.com',
        password_masked: 'my...rd',
        dav_type: 'caldav',
        sync_interval_minutes: 60,
        last_sync_at: null,
        sync_status: 'idle',
        sync_error: null,
        enabled: true,
      },
      {
        id: 'dav-2',
        name: 'iCloud',
        server_url: 'https://caldav.icloud.com',
        username: 'user@icloud.com',
        password_masked: 'ap...rd',
        dav_type: 'carddav',
        sync_interval_minutes: 120,
        last_sync_at: null,
        sync_status: 'error',
        sync_error: 'Connection timeout',
        enabled: false,
      },
    ]);
    render(<DavConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Radicale')).toBeInTheDocument();
      expect(screen.getByText('iCloud')).toBeInTheDocument();
    });
    expect(screen.getByText('Enabled')).toBeInTheDocument();
    expect(screen.getByText('Disabled')).toBeInTheDocument();
  });
});
