// Added: Unit tests for DelegationManager component (TMAIL-97)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DelegationManager } from './DelegationManager';

const mockListDelegations = vi.fn();
const mockListGrantedDelegations = vi.fn();
const mockGrantDelegation = vi.fn();
const mockRevokeDelegation = vi.fn();

vi.mock('../../api/delegation', () => ({
  listDelegations: () => mockListDelegations(),
  listGrantedDelegations: () => mockListGrantedDelegations(),
  grantDelegation: (...args: unknown[]) => mockGrantDelegation(...args),
  revokeDelegation: (...args: unknown[]) => mockRevokeDelegation(...args),
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

// Added: Sample delegation fixtures for testing
const sampleReceived = [
  {
    id: 'del-1',
    grantor_id: 'alice@example.com',
    delegate_id: 'me@example.com',
    delegation_type: 'send_as' as const,
    created_at: '2026-04-10T10:00:00Z',
  },
];

const sampleGranted = [
  {
    id: 'del-2',
    grantor_id: 'me@example.com',
    delegate_id: 'bob@example.com',
    delegation_type: 'send_on_behalf' as const,
    created_at: '2026-04-11T10:00:00Z',
  },
  {
    id: 'del-3',
    grantor_id: 'me@example.com',
    delegate_id: 'carol@example.com',
    delegation_type: 'send_as' as const,
    created_at: '2026-04-12T10:00:00Z',
  },
];

describe('DelegationManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and Grant Delegation button after loading', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Delegation')).toBeInTheDocument();
    });
    expect(screen.getByText(/Grant Delegation/)).toBeInTheDocument();
  });

  it('shows Received and Granted tabs', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Received')).toBeInTheDocument();
      expect(screen.getByText('Granted')).toBeInTheDocument();
    });
  });

  it('shows empty state when no received delegations exist', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText(
          'No delegations received. Other users can grant you permission to send as them.',
        ),
      ).toBeInTheDocument();
    });
  });

  it('renders received delegations with grantor info', async () => {
    mockListDelegations.mockResolvedValue(sampleReceived);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    });
    expect(screen.getByText('Send As')).toBeInTheDocument();
  });

  it('switches to granted tab and shows granted delegations', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue(sampleGranted);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Granted')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Granted'));

    await waitFor(() => {
      expect(screen.getByText('bob@example.com')).toBeInTheDocument();
      expect(screen.getByText('carol@example.com')).toBeInTheDocument();
    });
  });

  it('shows empty state on granted tab when no granted delegations exist', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Granted')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Granted'));

    await waitFor(() => {
      expect(
        screen.getByText(
          'No delegations granted. Grant delegation to allow others to send as you.',
        ),
      ).toBeInTheDocument();
    });
  });

  it('shows grant form when Grant Delegation is clicked', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/Grant Delegation/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/Grant Delegation/));

    expect(screen.getByText('Grant New Delegation')).toBeInTheDocument();
    expect(screen.getByTestId('delegate-email')).toBeInTheDocument();
    expect(screen.getByTestId('delegation-type')).toBeInTheDocument();
  });

  it('renders revoke buttons for granted delegations', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue(sampleGranted);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Granted')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Granted'));

    await waitFor(() => {
      expect(screen.getByTestId('revoke-del-2')).toBeInTheDocument();
      expect(screen.getByTestId('revoke-del-3')).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', async () => {
    mockListDelegations.mockResolvedValue([]);
    mockListGrantedDelegations.mockResolvedValue([]);
    render(<DelegationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
