import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { TwoFactorManager } from './TwoFactorManager';

const mockGetStatus = vi.fn();
const mockEnroll = vi.fn();
const mockVerify = vi.fn();
const mockDisable = vi.fn();

vi.mock('../../api/two-factor', () => ({
  twoFactorApi: {
    getStatus: (...args: unknown[]) => mockGetStatus(...args),
    enroll: (...args: unknown[]) => mockEnroll(...args),
    verify: (...args: unknown[]) => mockVerify(...args),
    disable: (...args: unknown[]) => mockDisable(...args),
  },
}));

function createWrapper() {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  );
}

describe('TwoFactorManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders "Two-Factor Authentication" heading', async () => {
    mockGetStatus.mockResolvedValue({ enabled: false });
    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Two-Factor Authentication')).toBeInTheDocument();
    });
  });

  it('shows "Enable 2FA" button when status.enabled is false', async () => {
    mockGetStatus.mockResolvedValue({ enabled: false });
    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Enable 2FA')).toBeInTheDocument();
    });
  });

  it('shows "2FA is enabled" message when status.enabled is true', async () => {
    mockGetStatus.mockResolvedValue({ enabled: true, backup_codes_remaining: 8 });
    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('2FA is enabled')).toBeInTheDocument();
    });
  });

  it('shows backup codes remaining count when enabled', async () => {
    mockGetStatus.mockResolvedValue({ enabled: true, backup_codes_remaining: 5 });
    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('5 backup codes remaining')).toBeInTheDocument();
    });
  });

  it('shows QR code step after enrollment', async () => {
    mockGetStatus.mockResolvedValue({ enabled: false });
    mockEnroll.mockResolvedValue({
      secret: 'JBSWY3DPEHPK3PXP',
      otpauth_url: 'otpauth://totp/TASMail?secret=JBSWY3DPEHPK3PXP',
      backup_codes: ['code1', 'code2', 'code3', 'code4'],
    });

    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Enable 2FA')).toBeInTheDocument();
    });

    screen.getByText('Enable 2FA').click();

    await waitFor(() => {
      expect(screen.getByText('Step 1: Scan QR Code')).toBeInTheDocument();
    });
    expect(screen.getByAltText('TOTP QR Code')).toBeInTheDocument();
    expect(screen.getByText('JBSWY3DPEHPK3PXP')).toBeInTheDocument();
    expect(screen.getByText('Step 2: Save Backup Codes')).toBeInTheDocument();
    expect(screen.getByText('Step 3: Verify')).toBeInTheDocument();
  });

  it('shows disable section with code input when enabled', async () => {
    mockGetStatus.mockResolvedValue({ enabled: true, backup_codes_remaining: 8 });
    render(<TwoFactorManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Disable 2FA')).toBeInTheDocument();
    });
    expect(screen.getByPlaceholderText('Enter 6-digit code')).toBeInTheDocument();
    expect(screen.getByText('Disable')).toBeInTheDocument();
  });
});
