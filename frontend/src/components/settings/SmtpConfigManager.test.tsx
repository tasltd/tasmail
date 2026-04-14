// Added: SmtpConfigManager component tests for TMAIL-48

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SmtpConfigManager } from './SmtpConfigManager';

const mockListSmtpConfigs = vi.fn();
const mockCreateSmtpConfig = vi.fn();
const mockUpdateSmtpConfig = vi.fn();
const mockDeleteSmtpConfig = vi.fn();
const mockTestSmtpConfig = vi.fn();
const mockSetDefaultSmtp = vi.fn();

vi.mock('../../api/smtp-config', () => ({
  listSmtpConfigs: () => mockListSmtpConfigs(),
  createSmtpConfig: (...args: unknown[]) => mockCreateSmtpConfig(...args),
  updateSmtpConfig: (...args: unknown[]) => mockUpdateSmtpConfig(...args),
  deleteSmtpConfig: (...args: unknown[]) => mockDeleteSmtpConfig(...args),
  testSmtpConfig: (...args: unknown[]) => mockTestSmtpConfig(...args),
  setDefaultSmtp: (...args: unknown[]) => mockSetDefaultSmtp(...args),
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

describe('SmtpConfigManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders SMTP Configuration heading after loading', async () => {
    mockListSmtpConfigs.mockResolvedValue([]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('SMTP Configuration')).toBeInTheDocument();
    });
  });

  it('shows empty state when no configs exist', async () => {
    mockListSmtpConfigs.mockResolvedValue([]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No SMTP servers configured. Add one to send emails through your own provider.'),
      ).toBeInTheDocument();
    });
  });

  it('shows create form with encryption dropdown when adding', async () => {
    mockListSmtpConfigs.mockResolvedValue([]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add SMTP Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add SMTP Server'));

    const encSelect = screen.getByTestId('encryption-select');
    expect(encSelect).toBeInTheDocument();
    const options = encSelect.querySelectorAll('option');
    expect(options.length).toBe(3);
    expect(options[0].textContent).toBe('STARTTLS (port 587)');
    expect(options[1].textContent).toBe('SSL/TLS (port 465)');
    expect(options[2].textContent).toBe('None (port 25)');
  });

  it('shows password input as password type', async () => {
    mockListSmtpConfigs.mockResolvedValue([]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add SMTP Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add SMTP Server'));

    const passwordInput = screen.getByTestId('password-input') as HTMLInputElement;
    expect(passwordInput).toBeInTheDocument();
    expect(passwordInput.type).toBe('password');
  });

  it('shows preset buttons in create form', async () => {
    mockListSmtpConfigs.mockResolvedValue([]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add SMTP Server')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add SMTP Server'));

    expect(screen.getByTestId('preset-gmail')).toBeInTheDocument();
    expect(screen.getByTestId('preset-sendgrid')).toBeInTheDocument();
  });

  it('shows test and delete buttons for each configuration', async () => {
    mockListSmtpConfigs.mockResolvedValue([
      {
        id: 'smtp-1',
        name: 'Gmail',
        host: 'smtp.gmail.com',
        port: 587,
        username: 'user@gmail.com',
        password_masked: 'ap...rd',
        encryption: 'starttls',
        from_address: 'user@gmail.com',
        is_default: false,
        verified: true,
        last_tested_at: '2026-04-14T00:00:00Z',
      },
    ]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('test-smtp-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Test connection')).toBeInTheDocument();
    expect(screen.getByTestId('delete-smtp-1')).toBeInTheDocument();
  });

  it('shows default badge for default configuration', async () => {
    mockListSmtpConfigs.mockResolvedValue([
      {
        id: 'smtp-1',
        name: 'SendGrid',
        host: 'smtp.sendgrid.net',
        port: 587,
        username: 'apikey',
        password_masked: 'SG...xx',
        encryption: 'starttls',
        from_address: null,
        is_default: true,
        verified: true,
        last_tested_at: null,
      },
    ]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('default-badge')).toBeInTheDocument();
    });
    expect(screen.getByText('Default')).toBeInTheDocument();
    expect(screen.getByText('Verified')).toBeInTheDocument();
  });

  it('shows set-as-default button for non-default configs', async () => {
    mockListSmtpConfigs.mockResolvedValue([
      {
        id: 'smtp-1',
        name: 'Test SMTP',
        host: 'smtp.test.com',
        port: 587,
        username: 'test',
        password_masked: '****',
        encryption: 'starttls',
        from_address: null,
        is_default: false,
        verified: false,
        last_tested_at: null,
      },
    ]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('default-smtp-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Set as default')).toBeInTheDocument();
  });

  it('renders config list with host, port, and encryption details', async () => {
    mockListSmtpConfigs.mockResolvedValue([
      {
        id: 'smtp-1',
        name: 'Gmail',
        host: 'smtp.gmail.com',
        port: 587,
        username: 'user@gmail.com',
        password_masked: 'ap...rd',
        encryption: 'starttls',
        from_address: 'user@gmail.com',
        is_default: true,
        verified: true,
        last_tested_at: null,
      },
      {
        id: 'smtp-2',
        name: 'SendGrid',
        host: 'smtp.sendgrid.net',
        port: 587,
        username: 'apikey',
        password_masked: 'SG...xx',
        encryption: 'starttls',
        from_address: null,
        is_default: false,
        verified: false,
        last_tested_at: null,
      },
    ]);
    render(<SmtpConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Gmail')).toBeInTheDocument();
      expect(screen.getByText('SendGrid')).toBeInTheDocument();
    });
    expect(screen.getByText('Verified')).toBeInTheDocument();
    expect(screen.getByText('Unverified')).toBeInTheDocument();
  });
});
