// Added: HostnameManager component tests for TMAIL-112

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { HostnameManager } from './HostnameManager';

const mockListHostnames = vi.fn();
const mockCreateHostname = vi.fn();
const mockDeleteHostname = vi.fn();
const mockVerifyHostname = vi.fn();

vi.mock('../../api/custom-hostnames', () => ({
  listHostnames: () => mockListHostnames(),
  createHostname: (...args: unknown[]) => mockCreateHostname(...args),
  deleteHostname: (...args: unknown[]) => mockDeleteHostname(...args),
  verifyHostname: (...args: unknown[]) => mockVerifyHostname(...args),
}));

// Added: Mock apiClient for the inline domain fetch
vi.mock('../../api/client', () => ({
  apiClient: {
    get: vi.fn().mockResolvedValue([
      { id: 'domain-1', name: 'acme.com', active: true },
      { id: 'domain-2', name: 'example.org', active: true },
    ]),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
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

describe('HostnameManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Custom Hostnames heading after loading', async () => {
    mockListHostnames.mockResolvedValue([]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Custom Hostnames')).toBeInTheDocument();
    });
  });

  it('shows empty state when no hostnames exist', async () => {
    mockListHostnames.mockResolvedValue([]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No custom hostnames configured. Add one to enable custom SMTP/IMAP domains.'),
      ).toBeInTheDocument();
    });
  });

  it('renders hostname list with SMTP and IMAP details', async () => {
    mockListHostnames.mockResolvedValue([
      {
        id: 'h-1',
        domain_id: 'domain-1',
        smtp_hostname: 'smtp.acme.com',
        imap_hostname: 'imap.acme.com',
        webmail_hostname: 'mail.acme.com',
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: true,
        verified_at: '2026-04-10T12:00:00Z',
        dns_verification_token: 'token-abc',
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
    ]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/smtp\.acme\.com/)).toBeInTheDocument();
      expect(screen.getByText(/imap\.acme\.com/)).toBeInTheDocument();
    });
  });

  it('shows add form when Add Hostname is clicked', async () => {
    mockListHostnames.mockResolvedValue([]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Hostname')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Hostname'));

    expect(screen.getByText('New Custom Hostname')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('smtp.example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('imap.example.com')).toBeInTheDocument();
  });

  it('shows verification status badges', async () => {
    mockListHostnames.mockResolvedValue([
      {
        id: 'h-verified',
        domain_id: 'domain-1',
        smtp_hostname: 'smtp.verified.com',
        imap_hostname: 'imap.verified.com',
        webmail_hostname: null,
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: true,
        verified_at: '2026-04-10T12:00:00Z',
        dns_verification_token: null,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
      {
        id: 'h-unverified',
        domain_id: 'domain-2',
        smtp_hostname: 'smtp.pending.com',
        imap_hostname: 'imap.pending.com',
        webmail_hostname: null,
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: false,
        verified_at: null,
        dns_verification_token: 'verify-token-123',
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
    ]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Verified')).toBeInTheDocument();
      expect(screen.getByText('Unverified')).toBeInTheDocument();
    });
  });

  it('shows verify button for unverified hostnames', async () => {
    mockListHostnames.mockResolvedValue([
      {
        id: 'h-unverified',
        domain_id: 'domain-1',
        smtp_hostname: 'smtp.pending.com',
        imap_hostname: 'imap.pending.com',
        webmail_hostname: null,
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: false,
        verified_at: null,
        dns_verification_token: 'verify-token-456',
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
    ]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('verify-h-unverified')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Verify DNS')).toBeInTheDocument();
  });

  it('renders domain dropdown in the add form', async () => {
    mockListHostnames.mockResolvedValue([]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Hostname')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Hostname'));

    await waitFor(() => {
      expect(screen.getByTestId('domain-select')).toBeInTheDocument();
    });
    expect(screen.getByText('Select a domain...')).toBeInTheDocument();
  });

  it('renders delete buttons for each hostname', async () => {
    mockListHostnames.mockResolvedValue([
      {
        id: 'h-1',
        domain_id: 'domain-1',
        smtp_hostname: 'smtp.one.com',
        imap_hostname: 'imap.one.com',
        webmail_hostname: null,
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: true,
        verified_at: '2026-04-10T12:00:00Z',
        dns_verification_token: null,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
      {
        id: 'h-2',
        domain_id: 'domain-2',
        smtp_hostname: 'smtp.two.com',
        imap_hostname: 'imap.two.com',
        webmail_hostname: null,
        autodiscover_hostname: null,
        tls_cert_path: null,
        tls_key_path: null,
        verified: false,
        verified_at: null,
        dns_verification_token: 'token-xyz',
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      },
    ]);
    render(<HostnameManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
  });
});
