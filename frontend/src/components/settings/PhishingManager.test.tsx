// Added: PhishingManager component tests for TMAIL-124

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PhishingManager } from './PhishingManager';

const mockGetPhishingReport = vi.fn();
const mockScanMessage = vi.fn();
const mockUpdatePhishingAction = vi.fn();

vi.mock('../../api/phishing', () => ({
  getPhishingReport: (...args: unknown[]) => mockGetPhishingReport(...args),
  scanMessage: (...args: unknown[]) => mockScanMessage(...args),
  updatePhishingAction: (...args: unknown[]) => mockUpdatePhishingAction(...args),
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

describe('PhishingManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Phishing Protection heading', () => {
    render(<PhishingManager />, { wrapper: createWrapper() });
    expect(screen.getByText('Phishing Protection')).toBeInTheDocument();
  });

  it('renders Scan Message and Lookup Report sections', () => {
    render(<PhishingManager />, { wrapper: createWrapper() });
    expect(screen.getByText('Scan Message')).toBeInTheDocument();
    expect(screen.getByText('Lookup Report')).toBeInTheDocument();
  });

  it('shows empty state when no report is loaded', () => {
    render(<PhishingManager />, { wrapper: createWrapper() });
    expect(
      screen.getByText('No phishing report loaded. Use the scan or lookup forms above to check a message.'),
    ).toBeInTheDocument();
  });

  it('renders scan form fields', () => {
    render(<PhishingManager />, { wrapper: createWrapper() });
    // NOTE: There are two "INBOX" placeholders (scan + lookup), so use getAllBy
    const inboxInputs = screen.getAllByPlaceholderText('INBOX');
    expect(inboxInputs.length).toBe(2);
    expect(screen.getByPlaceholderText('John Doe')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('sender@example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('<html>...</html>')).toBeInTheDocument();
  });

  it('shows phishing report after scan completes', async () => {
    mockScanMessage.mockResolvedValue({
      id: 'r1',
      mailbox_id: 'mb1',
      message_uid: 42,
      folder: 'INBOX',
      suspicious_links: [
        { url: 'http://evil.com', display_text: 'Click here', reasons: ['domain mismatch'] },
      ],
      suspicious_sender: true,
      spoofed_display_name: false,
      risk_score: 75,
      user_action: '',
      created_at: '2026-04-14T10:00:00Z',
    });

    render(<PhishingManager />, { wrapper: createWrapper() });

    // NOTE: Fill scan form fields — there are two "Folder" inputs, use the first one (scan form)
    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'INBOX' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '42' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'Attacker' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'bad@evil.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html><a href="http://evil.com">Click here</a></html>' } });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByTestId('phishing-report')).toBeInTheDocument();
    });

    expect(screen.getByText(/High Risk/)).toBeInTheDocument();
    expect(screen.getByText('http://evil.com')).toBeInTheDocument();
    expect(screen.getByText('domain mismatch')).toBeInTheDocument();
  });

  it('renders risk level badges correctly for different scores', async () => {
    mockScanMessage.mockResolvedValue({
      id: 'r2',
      mailbox_id: 'mb1',
      message_uid: 10,
      folder: 'INBOX',
      suspicious_links: [],
      suspicious_sender: false,
      spoofed_display_name: false,
      risk_score: 20,
      user_action: 'dismissed',
      created_at: '2026-04-14T10:00:00Z',
    });

    render(<PhishingManager />, { wrapper: createWrapper() });

    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'INBOX' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '10' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'Safe User' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'safe@good.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html>Hello</html>' } });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByText(/Low Risk/)).toBeInTheDocument();
    });
  });

  it('shows action buttons Safe, Dismiss, and Report', async () => {
    mockScanMessage.mockResolvedValue({
      id: 'r3',
      mailbox_id: 'mb1',
      message_uid: 5,
      folder: 'Sent',
      suspicious_links: [],
      suspicious_sender: false,
      spoofed_display_name: false,
      risk_score: 50,
      user_action: '',
      created_at: '2026-04-14T10:00:00Z',
    });

    render(<PhishingManager />, { wrapper: createWrapper() });

    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'Sent' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '5' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'Test' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'test@test.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html>x</html>' } });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByText('Safe')).toBeInTheDocument();
      expect(screen.getByText('Dismiss')).toBeInTheDocument();
      expect(screen.getByText('Report')).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', () => {
    render(<PhishingManager />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('renders dangerous attachments section when report has them (TMAIL-124)', async () => {
    // Added: TMAIL-124 — verify Safe-Attachments-style warnings render in the report
    mockScanMessage.mockResolvedValue({
      id: 'r-att',
      mailbox_id: 'mb1',
      message_uid: 11,
      folder: 'INBOX',
      suspicious_links: [],
      suspicious_sender: false,
      spoofed_display_name: false,
      risk_score: 60,
      user_action: '',
      dangerous_attachments: [
        { filename: 'open-me.exe', extension: 'exe', reason: 'Executable or scriptable file type \'.exe\' — high malware risk' },
        { filename: 'invoice.pdf.scr', extension: 'scr', reason: 'Deceptive double extension' },
      ],
      created_at: '2026-04-14T10:00:00Z',
    });

    render(<PhishingManager />, { wrapper: createWrapper() });

    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'INBOX' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '11' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'Acct' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'ap@vendor.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html>x</html>' } });
    // Added: Fill the new attachment names field — covers the parsing path too
    fireEvent.change(screen.getByPlaceholderText('invoice.pdf, statement.exe'), {
      target: { value: 'open-me.exe, invoice.pdf.scr' },
    });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByTestId('dangerous-attachments')).toBeInTheDocument();
    });

    // NOTE: Filename appears inside <code>; reason text appears in plain text
    expect(screen.getByText('open-me.exe')).toBeInTheDocument();
    expect(screen.getByText('invoice.pdf.scr')).toBeInTheDocument();
    expect(screen.getByText(/Dangerous Attachments \(2\)/)).toBeInTheDocument();

    // Added: Confirm the request payload included the parsed attachments
    expect(mockScanMessage).toHaveBeenCalledWith(
      'INBOX',
      11,
      expect.objectContaining({
        attachments: [
          { filename: 'open-me.exe' },
          { filename: 'invoice.pdf.scr' },
        ],
      }),
    );
  });

  it('omits the dangerous attachments section when none are reported (TMAIL-124)', async () => {
    // Added: TMAIL-124 — when scanner returns no dangerous attachments, section is hidden
    mockScanMessage.mockResolvedValue({
      id: 'r-clean',
      mailbox_id: 'mb1',
      message_uid: 12,
      folder: 'INBOX',
      suspicious_links: [],
      suspicious_sender: false,
      spoofed_display_name: false,
      risk_score: 0,
      user_action: '',
      dangerous_attachments: [],
      created_at: '2026-04-14T10:00:00Z',
    });

    render(<PhishingManager />, { wrapper: createWrapper() });

    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'INBOX' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '12' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'OK' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'ok@ok.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html>x</html>' } });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByTestId('phishing-report')).toBeInTheDocument();
    });

    expect(screen.queryByTestId('dangerous-attachments')).not.toBeInTheDocument();
  });

  it('calls updatePhishingAction when action button is clicked', async () => {
    mockScanMessage.mockResolvedValue({
      id: 'r4',
      mailbox_id: 'mb1',
      message_uid: 7,
      folder: 'INBOX',
      suspicious_links: [],
      suspicious_sender: false,
      spoofed_display_name: false,
      risk_score: 85,
      user_action: '',
      created_at: '2026-04-14T10:00:00Z',
    });
    mockUpdatePhishingAction.mockResolvedValue(undefined);

    render(<PhishingManager />, { wrapper: createWrapper() });

    const folderInputs = screen.getAllByPlaceholderText('INBOX');
    fireEvent.change(folderInputs[0], { target: { value: 'INBOX' } });
    const uidInputs = screen.getAllByPlaceholderText('1');
    fireEvent.change(uidInputs[0], { target: { value: '7' } });
    fireEvent.change(screen.getByPlaceholderText('John Doe'), { target: { value: 'Phisher' } });
    fireEvent.change(screen.getByPlaceholderText('sender@example.com'), { target: { value: 'phish@bad.com' } });
    fireEvent.change(screen.getByPlaceholderText('<html>...</html>'), { target: { value: '<html>phish</html>' } });

    fireEvent.click(screen.getByText('Scan'));

    await waitFor(() => {
      expect(screen.getByText('Report')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Report'));

    await waitFor(() => {
      expect(mockUpdatePhishingAction).toHaveBeenCalledWith('r4', { action: 'reported' });
    });
  });
});
