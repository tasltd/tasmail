// Added: Pop3ConfigManager component tests for TMAIL-133

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Pop3ConfigManager } from './Pop3ConfigManager';

const mockGetPop3Config = vi.fn();
const mockUpdatePop3Config = vi.fn();
const mockDeletePop3Config = vi.fn();
const mockGetPop3Status = vi.fn();

vi.mock('../../api/pop3-config', () => ({
  getPop3Config: () => mockGetPop3Config(),
  updatePop3Config: (...args: unknown[]) => mockUpdatePop3Config(...args),
  deletePop3Config: () => mockDeletePop3Config(),
  getPop3Status: () => mockGetPop3Status(),
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

describe('Pop3ConfigManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetPop3Status.mockResolvedValue({
      server: 'mail.example.com',
      port: 995,
      encryption: 'SSL/TLS',
      username_format: 'user@mail.example.com',
    });
  });

  it('renders POP3 Configuration heading after loading', async () => {
    mockGetPop3Config.mockResolvedValue(null);
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('POP3 Configuration')).toBeInTheDocument();
    });
  });

  it('shows connection info from status endpoint', async () => {
    mockGetPop3Config.mockResolvedValue(null);
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('pop3-server')).toHaveTextContent('mail.example.com');
    });
    expect(screen.getByTestId('pop3-port')).toHaveTextContent('995');
    expect(screen.getByTestId('pop3-encryption')).toHaveTextContent('SSL/TLS');
    expect(screen.getByTestId('pop3-username')).toHaveTextContent('user@mail.example.com');
  });

  it('shows POP3 settings form with checkboxes', async () => {
    mockGetPop3Config.mockResolvedValue(null);
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('enabled-toggle')).toBeInTheDocument();
    });
    expect(screen.getByTestId('delete-after-download-toggle')).toBeInTheDocument();
    expect(screen.getByTestId('retention-days-input')).toBeInTheDocument();
    expect(screen.getByTestId('save-btn')).toBeInTheDocument();
  });

  it('shows delete button when config exists', async () => {
    mockGetPop3Config.mockResolvedValue({
      id: 'pop3-1',
      user_id: 'user-1',
      enabled: true,
      delete_after_download: false,
      retention_days: null,
      last_pop3_login: null,
      created_at: '2026-04-14T00:00:00Z',
      updated_at: '2026-04-14T00:00:00Z',
    });
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('delete-btn')).toBeInTheDocument();
    });
  });

  it('does not show delete button when no config exists', async () => {
    mockGetPop3Config.mockResolvedValue(null);
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('save-btn')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('delete-btn')).not.toBeInTheDocument();
  });

  it('shows mail client connection info heading', async () => {
    mockGetPop3Config.mockResolvedValue(null);
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Mail Client Connection Info')).toBeInTheDocument();
    });
  });

  it('shows last POP3 login when available', async () => {
    mockGetPop3Config.mockResolvedValue({
      id: 'pop3-1',
      user_id: 'user-1',
      enabled: true,
      delete_after_download: false,
      retention_days: 30,
      last_pop3_login: '2026-04-13T10:00:00Z',
      created_at: '2026-04-14T00:00:00Z',
      updated_at: '2026-04-14T00:00:00Z',
    });
    render(<Pop3ConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/Last POP3 login:/)).toBeInTheDocument();
    });
  });
});
