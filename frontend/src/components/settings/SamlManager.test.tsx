// Added: SamlManager component tests for TMAIL-101
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SamlManager } from './SamlManager';

const mockListSamlConfigs = vi.fn();
const mockCreateSamlConfig = vi.fn();
const mockUpdateSamlConfig = vi.fn();
const mockDeleteSamlConfig = vi.fn();
const mockGetSamlLoginUrl = vi.fn();

vi.mock('../../api/saml', () => ({
  listSamlConfigs: () => mockListSamlConfigs(),
  createSamlConfig: (...args: unknown[]) => mockCreateSamlConfig(...args),
  updateSamlConfig: (...args: unknown[]) => mockUpdateSamlConfig(...args),
  deleteSamlConfig: (...args: unknown[]) => mockDeleteSamlConfig(...args),
  getSamlLoginUrl: (...args: unknown[]) => mockGetSamlLoginUrl(...args),
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
  name: 'Okta SSO',
  entity_id: 'https://okta.example.com/saml',
  sso_url: 'https://okta.example.com/sso/saml',
  slo_url: 'https://okta.example.com/slo/saml',
  certificate: 'MIICpDCCAYwCCQ...',
  name_id_format: 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress',
  attribute_mapping: { email: 'email', name: 'displayName' },
  active: true,
  created_at: '2026-04-01T00:00:00Z',
  updated_at: '2026-04-10T12:00:00Z',
};

describe('SamlManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading', async () => {
    mockListSamlConfigs.mockResolvedValue([]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('SAML Single Sign-On')).toBeInTheDocument();
    });
  });

  it('shows empty state when no configs exist', async () => {
    mockListSamlConfigs.mockResolvedValue([]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No SAML configurations yet.')).toBeInTheDocument();
    });
  });

  it('displays config list with name, entity ID, and SSO URL', async () => {
    mockListSamlConfigs.mockResolvedValue([mockConfig]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Okta SSO')).toBeInTheDocument();
    });
    expect(screen.getByText('https://okta.example.com/saml')).toBeInTheDocument();
    expect(screen.getByText('SSO: https://okta.example.com/sso/saml')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('shows add form when Add IdP is clicked', async () => {
    mockListSamlConfigs.mockResolvedValue([]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add IdP')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add IdP'));

    expect(screen.getByLabelText('Configuration Name')).toBeInTheDocument();
    expect(screen.getByLabelText('IdP Entity ID')).toBeInTheDocument();
    expect(screen.getByLabelText('SSO URL')).toBeInTheDocument();
    expect(screen.getByLabelText('SLO URL (optional)')).toBeInTheDocument();
  });

  it('shows certificate textarea in the form', async () => {
    mockListSamlConfigs.mockResolvedValue([]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add IdP')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add IdP'));

    const certField = screen.getByLabelText('IdP Certificate (X.509 PEM)');
    expect(certField).toBeInTheDocument();
    expect(certField.tagName).toBe('TEXTAREA');
  });

  it('shows attribute mapping JSON editor in the form', async () => {
    mockListSamlConfigs.mockResolvedValue([]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add IdP')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add IdP'));

    const mappingField = screen.getByLabelText('Attribute Mapping (JSON)');
    expect(mappingField).toBeInTheDocument();
    expect(mappingField).toHaveValue('{"email": "email", "name": "displayName"}');
  });

  it('shows Test SSO button for each config', async () => {
    mockListSamlConfigs.mockResolvedValue([mockConfig]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Test SSO')).toBeInTheDocument();
    });
  });

  it('shows active toggle button for each config', async () => {
    mockListSamlConfigs.mockResolvedValue([mockConfig]);
    render(<SamlManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const activeButton = screen.getByText('Active');
      expect(activeButton).toBeInTheDocument();
      expect(activeButton.tagName).toBe('BUTTON');
    });
  });
});
