// Added: DeliverabilityReport component tests for TMAIL-39

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { DeliverabilityReport } from './DeliverabilityReport';

// Added: Mock the mail store
const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      setViewMode: mockSetViewMode,
      viewMode: 'deliverability',
    }),
}));

// Added: Mock the deliverability API
const mockRunCheck = vi.fn();
const mockGetExternalTools = vi.fn();
vi.mock('../../api/deliverability', () => ({
  runDeliverabilityCheck: (...args: unknown[]) => mockRunCheck(...args),
  getExternalDeliverabilityTools: (...args: unknown[]) =>
    mockGetExternalTools(...args),
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('DeliverabilityReport', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the heading and domain input', () => {
    render(<DeliverabilityReport />, { wrapper });
    expect(screen.getByText('Email Deliverability Check')).toBeInTheDocument();
    expect(screen.getByTestId('domain-input')).toBeInTheDocument();
    expect(screen.getByTestId('run-check-btn')).toBeInTheDocument();
  });

  it('disables Run Check button when domain is empty', () => {
    render(<DeliverabilityReport />, { wrapper });
    const btn = screen.getByTestId('run-check-btn');
    expect(btn).toBeDisabled();
  });

  it('enables Run Check button when domain is entered', () => {
    render(<DeliverabilityReport />, { wrapper });
    const input = screen.getByTestId('domain-input');
    fireEvent.change(input, { target: { value: 'mail.example.com' } });
    expect(screen.getByTestId('run-check-btn')).not.toBeDisabled();
  });

  it('calls runDeliverabilityCheck on form submit', async () => {
    mockRunCheck.mockResolvedValue({
      domain: 'mail.example.com',
      checks: [
        { name: 'SPF Record', status: 'pass', details: 'v=spf1 found' },
      ],
      score: 100,
    });

    render(<DeliverabilityReport />, { wrapper });
    const input = screen.getByTestId('domain-input');
    fireEvent.change(input, { target: { value: 'mail.example.com' } });
    fireEvent.click(screen.getByTestId('run-check-btn'));

    await waitFor(() => {
      expect(mockRunCheck).toHaveBeenCalledWith('mail.example.com');
    });
  });

  it('displays score and check results after successful check', async () => {
    mockRunCheck.mockResolvedValue({
      domain: 'example.com',
      checks: [
        { name: 'SPF Record', status: 'pass', details: 'v=spf1 found' },
        { name: 'DKIM Record', status: 'fail', details: 'No DKIM record' },
        { name: 'DMARC Record', status: 'warn', details: 'p=none' },
      ],
      score: 50,
    });

    render(<DeliverabilityReport />, { wrapper });
    fireEvent.change(screen.getByTestId('domain-input'), { target: { value: 'example.com' } });
    fireEvent.click(screen.getByTestId('run-check-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('score-display')).toHaveTextContent('50');
    });

    expect(screen.getByTestId('report-results')).toBeInTheDocument();
    expect(screen.getByText('SPF Record')).toBeInTheDocument();
    expect(screen.getByText('DKIM Record')).toBeInTheDocument();
    expect(screen.getByText('DMARC Record')).toBeInTheDocument();
  });

  it('expands check details on click', async () => {
    mockRunCheck.mockResolvedValue({
      domain: 'example.com',
      checks: [
        { name: 'SPF Record', status: 'pass', details: 'v=spf1 include:_spf.google.com ~all' },
      ],
      score: 100,
    });

    render(<DeliverabilityReport />, { wrapper });
    fireEvent.change(screen.getByTestId('domain-input'), { target: { value: 'example.com' } });
    fireEvent.click(screen.getByTestId('run-check-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('check-item-0')).toBeInTheDocument();
    });

    // Added: Details should not be visible initially
    expect(screen.queryByTestId('check-details-0')).not.toBeInTheDocument();

    // Added: Click to expand
    fireEvent.click(screen.getByTestId('check-item-0'));
    expect(screen.getByTestId('check-details-0')).toHaveTextContent('v=spf1 include:_spf.google.com ~all');

    // Added: Click again to collapse
    fireEvent.click(screen.getByTestId('check-item-0'));
    expect(screen.queryByTestId('check-details-0')).not.toBeInTheDocument();
  });

  it('navigates back when Back button is clicked', () => {
    render(<DeliverabilityReport />, { wrapper });
    fireEvent.click(screen.getByText('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('shows error message on API failure', async () => {
    mockRunCheck.mockRejectedValue(new Error('Server unreachable'));

    render(<DeliverabilityReport />, { wrapper });
    fireEvent.change(screen.getByTestId('domain-input'), { target: { value: 'bad.example.com' } });
    fireEvent.click(screen.getByTestId('run-check-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('error-message')).toBeInTheDocument();
    });
  });

  it('displays correct pass count summary', async () => {
    mockRunCheck.mockResolvedValue({
      domain: 'example.com',
      checks: [
        { name: 'SPF', status: 'pass', details: 'ok' },
        { name: 'DKIM', status: 'pass', details: 'ok' },
        { name: 'DMARC', status: 'fail', details: 'no' },
      ],
      score: 67,
    });

    render(<DeliverabilityReport />, { wrapper });
    fireEvent.change(screen.getByTestId('domain-input'), { target: { value: 'example.com' } });
    fireEvent.click(screen.getByTestId('run-check-btn'));

    await waitFor(() => {
      expect(screen.getByText('2 of 3 checks passed')).toBeInTheDocument();
    });
  });

  // === TMAIL-39 — External deliverability tools panel ===
  describe('External Tools panel', () => {
    const sampleReport = {
      domain: 'mail.example.com',
      checks: [{ name: 'SPF Record', status: 'pass', details: 'v=spf1 found' }],
      score: 100,
    };

    const sampleTools = {
      mail_tester: {
        test_address: 'test-tasmail-abc123@mail-tester.com',
        report_url: 'https://www.mail-tester.com/test-tasmail-abc123',
        expires_in_minutes: 45,
        instructions:
          'Send a test email to this address from your TASMail account.',
      },
      google_postmaster: {
        dashboard_url:
          'https://postmaster.google.com/managedomains?domain=mail.example.com',
        instructions: 'Sign in with the Google account that owns this domain.',
      },
      providers: [
        {
          name: 'Gmail',
          spam_folder_label: 'Spam',
          instructions: 'Send to a Gmail seed account.',
        },
        {
          name: 'Outlook / Hotmail',
          spam_folder_label: 'Junk',
          instructions: 'Send to an outlook.com account.',
        },
        {
          name: 'Yahoo Mail',
          spam_folder_label: 'Bulk',
          instructions: 'Send to a Yahoo account.',
        },
        {
          name: 'ProtonMail',
          spam_folder_label: 'Spam',
          instructions: 'Send to a ProtonMail account.',
        },
      ],
    };

    async function runReportFirst() {
      mockRunCheck.mockResolvedValue(sampleReport);
      render(<DeliverabilityReport />, { wrapper });
      fireEvent.change(screen.getByTestId('domain-input'), {
        target: { value: 'mail.example.com' },
      });
      fireEvent.click(screen.getByTestId('run-check-btn'));
      await waitFor(() => {
        expect(screen.getByTestId('report-results')).toBeInTheDocument();
      });
    }

    it('renders the external tools panel after a report is loaded', async () => {
      await runReportFirst();
      expect(screen.getByTestId('external-tools-panel')).toBeInTheDocument();
      expect(screen.getByTestId('generate-external-tools-btn')).toBeInTheDocument();
    });

    it('calls getExternalDeliverabilityTools with the reported domain', async () => {
      await runReportFirst();
      mockGetExternalTools.mockResolvedValue(sampleTools);
      fireEvent.click(screen.getByTestId('generate-external-tools-btn'));
      await waitFor(() => {
        expect(mockGetExternalTools).toHaveBeenCalledWith('mail.example.com');
      });
    });

    it('renders mail-tester address, report link, postmaster link, and all four providers', async () => {
      await runReportFirst();
      mockGetExternalTools.mockResolvedValue(sampleTools);
      fireEvent.click(screen.getByTestId('generate-external-tools-btn'));

      await waitFor(() => {
        expect(screen.getByTestId('mail-tester-address')).toHaveValue(
          'test-tasmail-abc123@mail-tester.com',
        );
      });

      expect(screen.getByTestId('mail-tester-report-link')).toHaveAttribute(
        'href',
        'https://www.mail-tester.com/test-tasmail-abc123',
      );
      expect(screen.getByTestId('postmaster-link')).toHaveAttribute(
        'href',
        'https://postmaster.google.com/managedomains?domain=mail.example.com',
      );

      // TMAIL-39 — issue spec requires all four providers; assert each row is rendered.
      expect(screen.getByText('Gmail')).toBeInTheDocument();
      expect(screen.getByText('Outlook / Hotmail')).toBeInTheDocument();
      expect(screen.getByText('Yahoo Mail')).toBeInTheDocument();
      expect(screen.getByText('ProtonMail')).toBeInTheDocument();
    });

    it('copies the mail-tester address to the clipboard when the Copy button is clicked', async () => {
      const writeText = vi.fn().mockResolvedValue(undefined);
      Object.assign(navigator, { clipboard: { writeText } });

      await runReportFirst();
      mockGetExternalTools.mockResolvedValue(sampleTools);
      fireEvent.click(screen.getByTestId('generate-external-tools-btn'));
      await waitFor(() => {
        expect(screen.getByTestId('mail-tester-copy-btn')).toBeInTheDocument();
      });

      fireEvent.click(screen.getByTestId('mail-tester-copy-btn'));
      await waitFor(() => {
        expect(writeText).toHaveBeenCalledWith(
          'test-tasmail-abc123@mail-tester.com',
        );
      });
    });

    it('shows an error message when the external tools fetch fails', async () => {
      await runReportFirst();
      mockGetExternalTools.mockRejectedValue(new Error('upstream down'));
      fireEvent.click(screen.getByTestId('generate-external-tools-btn'));
      await waitFor(() => {
        expect(screen.getByTestId('external-tools-error')).toBeInTheDocument();
      });
    });
  });
});
