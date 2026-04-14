// Added: OidcManager component tests for TMAIL-99
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { OidcManager } from './OidcManager';

const mockListOidcProviders = vi.fn();
const mockCreateOidcProvider = vi.fn();
const mockUpdateOidcProvider = vi.fn();
const mockDeleteOidcProvider = vi.fn();

vi.mock('../../api/oidc', () => ({
  listOidcProviders: () => mockListOidcProviders(),
  createOidcProvider: (...args: unknown[]) => mockCreateOidcProvider(...args),
  updateOidcProvider: (...args: unknown[]) => mockUpdateOidcProvider(...args),
  deleteOidcProvider: (...args: unknown[]) => mockDeleteOidcProvider(...args),
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

const mockProvider = {
  id: 'prov-1',
  name: 'Google',
  issuer_url: 'https://accounts.google.com',
  client_id: '123456.apps.googleusercontent.com',
  scopes: 'openid email profile',
  redirect_uri: 'https://mail.example.com/api/auth/oidc/callback',
  auto_create_users: true,
  default_role: 'user',
  active: true,
  icon_url: 'https://cdn.example.com/google.svg',
  button_label: 'Sign in with Google',
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-10T12:00:00Z',
};

describe('OidcManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('OIDC Providers')).toBeInTheDocument();
    });
  });

  it('shows empty state when no providers exist', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No OIDC providers configured yet.')).toBeInTheDocument();
    });
  });

  it('displays provider list with name and issuer URL', async () => {
    mockListOidcProviders.mockResolvedValue([mockProvider]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Google')).toBeInTheDocument();
    });
    expect(screen.getByText('https://accounts.google.com')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Auto-create')).toBeInTheDocument();
  });

  it('shows add form when Add Provider is clicked', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    expect(screen.getByLabelText('Provider Name')).toBeInTheDocument();
    expect(screen.getByLabelText('Issuer URL')).toBeInTheDocument();
    expect(screen.getByLabelText('Client ID')).toBeInTheDocument();
    expect(screen.getByLabelText('Redirect URI')).toBeInTheDocument();
  });

  it('shows client secret as password input', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    const secretInput = screen.getByLabelText('Client Secret');
    expect(secretInput).toBeInTheDocument();
    expect(secretInput).toHaveAttribute('type', 'password');
  });

  it('shows scopes field with default value', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    const scopesInput = screen.getByLabelText('Scopes');
    expect(scopesInput).toBeInTheDocument();
    expect(scopesInput).toHaveValue('openid email profile');
  });

  it('shows auto-create users toggle', async () => {
    mockListOidcProviders.mockResolvedValue([]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    expect(screen.getByText('Auto-create users on first login')).toBeInTheDocument();
    // Added: Verify the checkbox is unchecked by default
    const checkbox = screen.getByRole('checkbox');
    expect(checkbox).not.toBeChecked();
  });

  it('shows active status toggle for each provider', async () => {
    mockListOidcProviders.mockResolvedValue([mockProvider]);
    render(<OidcManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Google')).toBeInTheDocument();
    });

    // Added: Verify active/deactivate toggle button exists
    expect(screen.getByTitle('Deactivate')).toBeInTheDocument();
  });
});
