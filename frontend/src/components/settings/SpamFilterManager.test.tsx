// Added: Unit tests for SpamFilterManager component (TMAIL-15)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SpamFilterManager } from './SpamFilterManager';

const mockFetchSpamSettings = vi.fn();
const mockUpdateSpamSettings = vi.fn();
const mockFetchQuarantine = vi.fn();
const mockReleaseQuarantine = vi.fn();
const mockDeleteQuarantine = vi.fn();
const mockFetchSpamStats = vi.fn();

vi.mock('../../api/spam', () => ({
  fetchSpamSettings: (...args: unknown[]) => mockFetchSpamSettings(...args),
  updateSpamSettings: (...args: unknown[]) => mockUpdateSpamSettings(...args),
  fetchQuarantine: () => mockFetchQuarantine(),
  releaseQuarantine: (...args: unknown[]) => mockReleaseQuarantine(...args),
  deleteQuarantine: (...args: unknown[]) => mockDeleteQuarantine(...args),
  fetchSpamStats: () => mockFetchSpamStats(),
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

describe('SpamFilterManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton while fetching', () => {
    mockFetchSpamSettings.mockReturnValue(new Promise(() => {}));
    mockFetchQuarantine.mockReturnValue(new Promise(() => {}));
    mockFetchSpamStats.mockReturnValue(new Promise(() => {}));
    render(<SpamFilterManager />, { wrapper: createWrapper() });
    // NOTE: LoadingSkeleton renders placeholder divs, header not yet visible
    expect(screen.queryByText('Spam Filter')).not.toBeInTheDocument();
  });

  it('renders header and tab navigation', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 0, total_blocked: 0, total_passed: 0, quarantined: 0, released: 0 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Spam Filter')).toBeInTheDocument();
    });
    expect(screen.getByText('settings')).toBeInTheDocument();
    expect(screen.getByText('quarantine')).toBeInTheDocument();
    expect(screen.getByText('statistics')).toBeInTheDocument();
  });

  it('renders settings tab with threshold sliders', async () => {
    mockFetchSpamSettings.mockResolvedValue({
      id: 'abc',
      threshold_reject: 15,
      threshold_greylist: 4,
      threshold_add_header: 6,
      dkim_signing_enabled: true,
      arc_signing_enabled: false,
      autolearn_enabled: true,
    });
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 0, total_blocked: 0, total_passed: 0, quarantined: 0, released: 0 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/Reject Threshold/)).toBeInTheDocument();
    });
    expect(screen.getByText(/Greylist Threshold/)).toBeInTheDocument();
    expect(screen.getByText(/Add Header Threshold/)).toBeInTheDocument();
    expect(screen.getByText('DKIM Signing')).toBeInTheDocument();
    expect(screen.getByText('ARC Signing')).toBeInTheDocument();
    expect(screen.getByText('Autolearn')).toBeInTheDocument();
    expect(screen.getByText('Save Settings')).toBeInTheDocument();
  });

  it('renders quarantine tab with empty state', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 0, total_blocked: 0, total_passed: 0, quarantined: 0, released: 0 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Spam Filter')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('quarantine'));
    expect(screen.getByText('No quarantined messages.')).toBeInTheDocument();
  });

  it('renders quarantine items with release and delete buttons', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([
      {
        id: '1',
        user_id: 'u1',
        message_id: 'msg-1',
        sender: 'spammer@bad.com',
        subject: 'Buy cheap stuff',
        score: 18.5,
        action: 'reject',
        symbols: [],
        quarantined_at: '2026-04-14T10:00:00Z',
        released: false,
        released_at: null,
      },
    ]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 100, total_blocked: 10, total_passed: 90, quarantined: 5, released: 2 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Spam Filter')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('quarantine'));

    await waitFor(() => {
      expect(screen.getByText('Buy cheap stuff')).toBeInTheDocument();
    });
    expect(screen.getByText(/spammer@bad.com/)).toBeInTheDocument();
    expect(screen.getByTitle('Release')).toBeInTheDocument();
    expect(screen.getByTitle('Delete')).toBeInTheDocument();
  });

  it('renders statistics tab with stat cards', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({
      total_scanned: 5000,
      total_blocked: 500,
      total_passed: 4500,
      quarantined: 100,
      released: 20,
    });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Spam Filter')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('statistics'));

    await waitFor(() => {
      expect(screen.getByText('5,000')).toBeInTheDocument();
    });
    expect(screen.getByText('Total Scanned')).toBeInTheDocument();
    expect(screen.getByText('Blocked')).toBeInTheDocument();
  });

  it('navigates back when back button is clicked', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 0, total_blocked: 0, total_passed: 0, quarantined: 0, released: 0 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('renders refresh button', async () => {
    mockFetchSpamSettings.mockResolvedValue(null);
    mockFetchQuarantine.mockResolvedValue([]);
    mockFetchSpamStats.mockResolvedValue({ total_scanned: 0, total_blocked: 0, total_passed: 0, quarantined: 0, released: 0 });
    render(<SpamFilterManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Refresh')).toBeInTheDocument();
    });
  });
});
