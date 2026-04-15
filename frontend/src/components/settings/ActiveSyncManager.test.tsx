// Added: ActiveSyncManager component tests for TMAIL-130

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ActiveSyncManager } from './ActiveSyncManager';

const mockListDevices = vi.fn();
const mockBlockDevice = vi.fn();
const mockAllowDevice = vi.fn();
const mockWipeDevice = vi.fn();
const mockDeleteDevice = vi.fn();
const mockListPolicies = vi.fn();
const mockCreatePolicy = vi.fn();
const mockUpdatePolicy = vi.fn();
const mockDeletePolicy = vi.fn();

vi.mock('../../api/activesync', () => ({
  listDevices: () => mockListDevices(),
  blockDevice: (...args: unknown[]) => mockBlockDevice(...args),
  allowDevice: (...args: unknown[]) => mockAllowDevice(...args),
  wipeDevice: (...args: unknown[]) => mockWipeDevice(...args),
  deleteDevice: (...args: unknown[]) => mockDeleteDevice(...args),
  listPolicies: () => mockListPolicies(),
  createPolicy: (...args: unknown[]) => mockCreatePolicy(...args),
  updatePolicy: (...args: unknown[]) => mockUpdatePolicy(...args),
  deletePolicy: (...args: unknown[]) => mockDeletePolicy(...args),
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

describe('ActiveSyncManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListDevices.mockResolvedValue([]);
    mockListPolicies.mockResolvedValue([]);
  });

  it('renders ActiveSync Devices heading', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('ActiveSync Devices')).toBeInTheDocument();
    });
  });

  it('shows Devices and Policies tabs', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('devices-tab')).toBeInTheDocument();
    });
    expect(screen.getByTestId('policies-tab')).toBeInTheDocument();
  });

  it('shows empty message when no devices', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('no-devices-message')).toBeInTheDocument();
    });
  });

  it('shows device list with device data', async () => {
    mockListDevices.mockResolvedValue([
      {
        id: 'dev-1',
        user_id: 'user-1',
        device_id: 'IPHONE123',
        device_type: 'iPhone',
        device_name: 'Work iPhone',
        device_os: 'iOS 18',
        last_sync_at: '2026-04-14T10:00:00Z',
        status: 'allowed',
        policy_key: null,
        created_at: '2026-04-14T00:00:00Z',
        updated_at: '2026-04-14T00:00:00Z',
      },
    ]);

    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('device-dev-1')).toBeInTheDocument();
    });
    expect(screen.getByText('IPHONE123')).toBeInTheDocument();
    expect(screen.getByText('Work iPhone')).toBeInTheDocument();
    expect(screen.getByText('iOS 18')).toBeInTheDocument();
    expect(screen.getByTestId('status-dev-1')).toHaveTextContent('allowed');
  });

  it('shows block and wipe buttons for allowed device', async () => {
    mockListDevices.mockResolvedValue([
      {
        id: 'dev-1',
        user_id: 'user-1',
        device_id: 'DEV1',
        device_type: 'Android',
        device_name: null,
        device_os: null,
        last_sync_at: null,
        status: 'allowed',
        policy_key: null,
        created_at: null,
        updated_at: null,
      },
    ]);

    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('block-dev-1')).toBeInTheDocument();
    });
    expect(screen.getByTestId('wipe-dev-1')).toBeInTheDocument();
    // NOTE: Should NOT show allow button for already-allowed device
    expect(screen.queryByTestId('allow-dev-1')).not.toBeInTheDocument();
  });

  it('shows allow button for blocked device', async () => {
    mockListDevices.mockResolvedValue([
      {
        id: 'dev-2',
        user_id: 'user-1',
        device_id: 'DEV2',
        device_type: 'WindowsMail',
        device_name: null,
        device_os: null,
        last_sync_at: null,
        status: 'blocked',
        policy_key: null,
        created_at: null,
        updated_at: null,
      },
    ]);

    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('allow-dev-2')).toBeInTheDocument();
    });
    // NOTE: Should NOT show block button for already-blocked device
    expect(screen.queryByTestId('block-dev-2')).not.toBeInTheDocument();
  });

  it('switches to policies tab and shows empty state', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('policies-tab')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('policies-tab'));

    await waitFor(() => {
      expect(screen.getByTestId('policies-panel')).toBeInTheDocument();
    });
    expect(screen.getByTestId('no-policies-message')).toBeInTheDocument();
    expect(screen.getByTestId('add-policy-btn')).toBeInTheDocument();
  });

  it('shows policy form when Add Policy is clicked', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('policies-tab')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('policies-tab'));
    await waitFor(() => {
      expect(screen.getByTestId('add-policy-btn')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('add-policy-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('policy-form')).toBeInTheDocument();
    });
    expect(screen.getByTestId('policy-name-input')).toBeInTheDocument();
    expect(screen.getByTestId('require-encryption-toggle')).toBeInTheDocument();
    expect(screen.getByTestId('save-policy-btn')).toBeInTheDocument();
    expect(screen.getByTestId('cancel-policy-btn')).toBeInTheDocument();
  });

  it('shows policies in list with default badge', async () => {
    mockListPolicies.mockResolvedValue([
      {
        id: 'pol-1',
        name: 'Corporate Policy',
        require_encryption: true,
        max_inactivity_lock_mins: 5,
        min_password_length: 8,
        allow_simple_password: false,
        max_failed_password_attempts: 10,
        is_default: true,
        created_at: '2026-04-14T00:00:00Z',
      },
    ]);

    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('policies-tab')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('policies-tab'));

    await waitFor(() => {
      expect(screen.getByTestId('policy-pol-1')).toBeInTheDocument();
    });
    expect(screen.getByText('Corporate Policy')).toBeInTheDocument();
    expect(screen.getByTestId('default-pol-1')).toHaveTextContent('Default');
  });

  it('navigates back when back button is clicked', async () => {
    render(<ActiveSyncManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('back-btn')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('back-btn'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
