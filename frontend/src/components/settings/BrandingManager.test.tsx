// Added: BrandingManager component tests for TMAIL-111
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BrandingManager } from './BrandingManager';

const mockGetBranding = vi.fn();
const mockUpdateBranding = vi.fn();
const mockResetBranding = vi.fn();

vi.mock('../../api/branding', () => ({
  getBranding: () => mockGetBranding(),
  updateBranding: (...args: unknown[]) => mockUpdateBranding(...args),
  resetBranding: () => mockResetBranding(),
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

const defaultBranding = {
  id: 'test-id',
  app_name: 'TASMail',
  logo_url: 'https://example.com/logo.png',
  favicon_url: null,
  primary_color: '#2563eb',
  secondary_color: '#1e40af',
  accent_color: '#3b82f6',
  login_background_url: null,
  custom_css: 'body { color: red; }',
  footer_text: 'Powered by TASMail',
  support_email: 'help@example.com',
  support_url: 'https://support.example.com',
  updated_at: '2026-01-01T00:00:00Z',
};

describe('BrandingManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Branding')).toBeInTheDocument();
    });
  });

  it('shows app name input', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText('App Name')).toBeInTheDocument();
    });
    expect(screen.getByLabelText('App Name')).toHaveValue('TASMail');
  });

  it('shows color inputs', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText('Primary Color')).toBeInTheDocument();
    });
    expect(screen.getByLabelText('Secondary Color')).toBeInTheDocument();
    expect(screen.getByLabelText('Accent Color')).toBeInTheDocument();
  });

  it('shows logo preview when logo URL is set', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByAltText('Logo preview')).toBeInTheDocument();
    });
    expect(screen.getByAltText('Logo preview')).toHaveAttribute('src', 'https://example.com/logo.png');
  });

  it('shows custom CSS textarea', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText('Custom CSS')).toBeInTheDocument();
    });
    expect(screen.getByLabelText('Custom CSS')).toHaveValue('body { color: red; }');
  });

  it('shows footer text input', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByLabelText('Footer Text')).toBeInTheDocument();
    });
    expect(screen.getByLabelText('Footer Text')).toHaveValue('Powered by TASMail');
  });

  it('shows save button', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Save Branding')).toBeInTheDocument();
    });
  });

  it('shows reset button', async () => {
    mockGetBranding.mockResolvedValue(defaultBranding);
    render(<BrandingManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Reset to Defaults')).toBeInTheDocument();
    });
  });
});
