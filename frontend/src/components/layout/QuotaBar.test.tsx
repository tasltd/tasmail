import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { QuotaBar } from './QuotaBar';

// Mock the quota API
const mockGetQuota = vi.fn();
vi.mock('../../api/quota', () => ({
  quotaApi: {
    getQuota: () => mockGetQuota(),
  },
}));

function createWrapper() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
  };
}

// Helper to build quota data
function makeQuota(overrides: Partial<{
  quota_bytes: number;
  used_bytes: number;
  usage_percent: number;
  is_over_quota: boolean;
  is_warning: boolean;
}> = {}) {
  return {
    mailbox_id: 'test-mailbox',
    quota_bytes: 1073741824, // 1 GB
    used_bytes: 536870912, // 512 MB
    message_count: 100,
    usage_percent: 50,
    quota_warn_percent: 80,
    is_over_quota: false,
    is_warning: false,
    last_synced_at: '2026-04-10T00:00:00Z',
    ...overrides,
  };
}

describe('QuotaBar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders nothing when no quota data is available', async () => {
    mockGetQuota.mockResolvedValue(undefined);
    const { container } = render(<QuotaBar />, { wrapper: createWrapper() });
    // Component returns null when no data — container should have no quota-bar div
    // Added: wait for query to settle then assert empty
    await vi.waitFor(() => {
      expect(container.querySelector('.quota-bar')).not.toBeInTheDocument();
    });
  });

  it('shows used and total storage info', async () => {
    mockGetQuota.mockResolvedValue(makeQuota());
    render(<QuotaBar />, { wrapper: createWrapper() });
    await vi.waitFor(() => {
      expect(screen.getByText('512.0 MB used')).toBeInTheDocument();
      expect(screen.getByText('1.0 GB')).toBeInTheDocument();
    });
  });

  it('shows over-quota error message when is_over_quota is true', async () => {
    mockGetQuota.mockResolvedValue(
      makeQuota({ is_over_quota: true, usage_percent: 105 }),
    );
    render(<QuotaBar />, { wrapper: createWrapper() });
    await vi.waitFor(() => {
      expect(
        screen.getByText('Mailbox full — delete messages to free space'),
      ).toBeInTheDocument();
    });
  });

  it('shows warning message when is_warning is true but not over quota', async () => {
    mockGetQuota.mockResolvedValue(
      makeQuota({ is_warning: true, is_over_quota: false, usage_percent: 85 }),
    );
    render(<QuotaBar />, { wrapper: createWrapper() });
    await vi.waitFor(() => {
      expect(screen.getByText('85% of storage used')).toBeInTheDocument();
    });
  });

  it('does not show warning text when is_warning is true AND is_over_quota is true', async () => {
    mockGetQuota.mockResolvedValue(
      makeQuota({ is_warning: true, is_over_quota: true, usage_percent: 105 }),
    );
    render(<QuotaBar />, { wrapper: createWrapper() });
    await vi.waitFor(() => {
      // Over-quota message should show, not the warning percentage
      expect(
        screen.getByText('Mailbox full — delete messages to free space'),
      ).toBeInTheDocument();
      expect(screen.queryByText(/of storage used/)).not.toBeInTheDocument();
    });
  });

  it('renders a progress bar element', async () => {
    mockGetQuota.mockResolvedValue(makeQuota({ usage_percent: 50 }));
    const { container } = render(<QuotaBar />, { wrapper: createWrapper() });
    await vi.waitFor(() => {
      const quotaBar = container.querySelector('.quota-bar');
      expect(quotaBar).toBeInTheDocument();
    });
  });
});
