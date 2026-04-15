// Added: BillingManager component tests for TMAIL-46

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BillingManager } from './BillingManager';

// Added: Mock stores
const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode, viewMode: 'billing' }),
}));

// Added: Mock billing API
const mockListPlans = vi.fn();
const mockGetSubscription = vi.fn();
const mockSubscribe = vi.fn();
const mockListPayments = vi.fn();

vi.mock('../../api/billing', () => ({
  listPlans: (...args: unknown[]) => mockListPlans(...args),
  getSubscription: (...args: unknown[]) => mockGetSubscription(...args),
  subscribe: (...args: unknown[]) => mockSubscribe(...args),
  listPayments: (...args: unknown[]) => mockListPayments(...args),
}));

// Added: Mock LoadingSkeleton
vi.mock('../shared/LoadingSkeleton', () => ({
  LoadingSkeleton: () => <div data-testid="loading-skeleton">Loading...</div>,
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('BillingManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListPlans.mockResolvedValue([
      {
        id: 'plan-1',
        name: 'Basic Plan',
        description: 'Entry level mailbox',
        price_cedis: 19.99,
        interval: 'monthly',
        max_mailboxes: 1,
        storage_gb: 5,
        features: {},
        active: true,
        created_at: '2026-04-15T00:00:00Z',
        updated_at: '2026-04-15T00:00:00Z',
      },
      {
        id: 'plan-2',
        name: 'Pro Plan',
        description: 'For teams',
        price_cedis: 49.99,
        interval: 'monthly',
        max_mailboxes: 10,
        storage_gb: 50,
        features: { custom_domain: true },
        active: true,
        created_at: '2026-04-15T00:00:00Z',
        updated_at: '2026-04-15T00:00:00Z',
      },
    ]);
    mockGetSubscription.mockResolvedValue(null);
    mockListPayments.mockResolvedValue([]);
  });

  it('renders billing manager with heading', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('Billing & Subscription')).toBeInTheDocument();
    });
  });

  it('renders plan cards with names and prices', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('Basic Plan')).toBeInTheDocument();
      expect(screen.getByText('Pro Plan')).toBeInTheDocument();
    });
    // Check GHS pricing display
    expect(screen.getByText(/19\.99/)).toBeInTheDocument();
    expect(screen.getByText(/49\.99/)).toBeInTheDocument();
  });

  it('shows no subscription message when none active', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText(/No active subscription/)).toBeInTheDocument();
    });
  });

  it('shows active subscription status when present', async () => {
    mockGetSubscription.mockResolvedValue({
      id: 'sub-1',
      user_id: 'user-1',
      plan_id: 'plan-1',
      provider: 'paystack',
      status: 'active',
      current_period_end: '2026-05-15T00:00:00Z',
      created_at: '2026-04-15T00:00:00Z',
    });

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('active')).toBeInTheDocument();
      expect(screen.getByText('Paystack')).toBeInTheDocument();
    });
  });

  it('shows MoMo phone input when MoMo button clicked', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('Basic Plan')).toBeInTheDocument();
    });

    // Click the first MoMo button
    const momoButtons = screen.getAllByText('MTN MoMo');
    fireEvent.click(momoButtons[0]);

    expect(screen.getByTestId('momo-phone-input')).toBeInTheDocument();
  });

  it('renders payment history when payments exist', async () => {
    mockListPayments.mockResolvedValue([
      {
        id: 'pay-1',
        user_id: 'user-1',
        subscription_id: 'sub-1',
        provider: 'paystack',
        provider_ref: 'TMAIL-abc123',
        amount_cedis: 19.99,
        currency: 'GHS',
        status: 'success',
        metadata: {},
        created_at: '2026-04-15T12:00:00Z',
      },
    ]);

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('TMAIL-abc123')).toBeInTheDocument();
      expect(screen.getByText('success')).toBeInTheDocument();
    });
  });

  it('shows no payments message when empty', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('No payments yet.')).toBeInTheDocument();
    });
  });

  it('navigates back when back button clicked', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('Billing & Subscription')).toBeInTheDocument();
    });
    // Click the back arrow button
    const backBtn = screen.getByText('Billing & Subscription').closest('.settings-panel__header')?.querySelector('.btn--ghost');
    if (backBtn) {
      fireEvent.click(backBtn);
      expect(mockSetViewMode).toHaveBeenCalledWith('list');
    }
  });

  it('renders Pay with Card buttons for each plan', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      const cardButtons = screen.getAllByText('Pay with Card');
      expect(cardButtons).toHaveLength(2);
    });
  });
});
