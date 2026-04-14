// Added: DaneManager component tests for TMAIL-125

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DaneManager } from './DaneManager';

const mockListDanePolicies = vi.fn();
const mockCreateDanePolicy = vi.fn();
const mockDeleteDanePolicy = vi.fn();
const mockLookupTlsa = vi.fn();
const mockListDaneVerifications = vi.fn();

vi.mock('../../api/dane', () => ({
  listDanePolicies: () => mockListDanePolicies(),
  createDanePolicy: (...args: unknown[]) => mockCreateDanePolicy(...args),
  deleteDanePolicy: (...args: unknown[]) => mockDeleteDanePolicy(...args),
  lookupTlsa: (...args: unknown[]) => mockLookupTlsa(...args),
  listDaneVerifications: () => mockListDaneVerifications(),
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

describe('DaneManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders DANE / TLSA heading after loading', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('DANE / TLSA')).toBeInTheDocument();
    });
  });

  it('shows Policies and Verifications tabs', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Policies')).toBeInTheDocument();
      expect(screen.getByText('Verifications')).toBeInTheDocument();
    });
  });

  it('shows empty state when no policies exist', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No DANE policies configured. Add one to enable DANE verification for a domain.'),
      ).toBeInTheDocument();
    });
  });

  it('renders policy list with domain and enforce status', async () => {
    mockListDanePolicies.mockResolvedValue([
      {
        id: 'p1',
        domain: 'example.com',
        enforce: true,
        last_checked_at: '2026-04-10T12:00:00Z',
        tlsa_records: [{ usage: 3, selector: 1, matching_type: 1, cert_data: 'abcdef' }],
        created_at: '2026-04-10T10:00:00Z',
        updated_at: '2026-04-10T10:00:00Z',
      },
      {
        id: 'p2',
        domain: 'test.org',
        enforce: false,
        last_checked_at: null,
        tlsa_records: [],
        created_at: '2026-04-10T10:00:00Z',
        updated_at: '2026-04-10T10:00:00Z',
      },
    ]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('example.com')).toBeInTheDocument();
      expect(screen.getByText('test.org')).toBeInTheDocument();
    });
    expect(screen.getByText('Enforcing')).toBeInTheDocument();
    expect(screen.getByText('Monitor')).toBeInTheDocument();
  });

  it('shows add policy form when Add Policy is clicked', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Policy')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Policy'));

    expect(screen.getByText('New DANE Policy')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('example.com')).toBeInTheDocument();
  });

  it('shows TLSA Lookup section in policies tab', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('TLSA Lookup')).toBeInTheDocument();
      expect(screen.getByPlaceholderText('mail.example.com')).toBeInTheDocument();
    });
  });

  it('renders delete buttons for each policy', async () => {
    mockListDanePolicies.mockResolvedValue([
      {
        id: 'p1',
        domain: 'example.com',
        enforce: true,
        last_checked_at: null,
        tlsa_records: [],
        created_at: '2026-04-10T10:00:00Z',
        updated_at: '2026-04-10T10:00:00Z',
      },
      {
        id: 'p2',
        domain: 'test.org',
        enforce: false,
        last_checked_at: null,
        tlsa_records: [],
        created_at: '2026-04-10T10:00:00Z',
        updated_at: '2026-04-10T10:00:00Z',
      },
    ]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
  });

  it('switches to verifications tab and shows empty state', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    mockListDaneVerifications.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Verifications')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Verifications'));

    await waitFor(() => {
      expect(
        screen.getByText('No DANE verifications yet. Verifications will appear after sending emails to DANE-enabled domains.'),
      ).toBeInTheDocument();
    });
  });

  it('renders verifications table when data exists', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    mockListDaneVerifications.mockResolvedValue([
      {
        id: 'v1',
        user_id: 'u1',
        message_id: '<msg123@example.com>',
        recipient_domain: 'secure.org',
        dane_status: 'verified',
        checked_at: '2026-04-14T10:00:00Z',
      },
      {
        id: 'v2',
        user_id: 'u1',
        message_id: '<msg456@example.com>',
        recipient_domain: 'weak.org',
        dane_status: 'failed',
        checked_at: '2026-04-14T11:00:00Z',
      },
    ]);
    render(<DaneManager />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByText('Verifications'));

    await waitFor(() => {
      expect(screen.getByText('secure.org')).toBeInTheDocument();
      expect(screen.getByText('weak.org')).toBeInTheDocument();
      expect(screen.getByText('verified')).toBeInTheDocument();
      expect(screen.getByText('failed')).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', async () => {
    mockListDanePolicies.mockResolvedValue([]);
    render(<DaneManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });
});
