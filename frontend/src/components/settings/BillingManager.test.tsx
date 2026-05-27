// Changed: BillingManager component tests now match PayPro's provider set —
// Paystack, Mastercard, Cybersource, Bank Transfer (TMAIL-46). MoMo removed.

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

// Added: Mock billing API — keep label helpers real so they exercise the production code path.
const mockListPlans = vi.fn();
const mockGetSubscription = vi.fn();
const mockSubscribe = vi.fn();
const mockListPayments = vi.fn();

vi.mock('../../api/billing', async () => {
  const actual = await vi.importActual<typeof import('../../api/billing')>('../../api/billing');
  return {
    ...actual,
    listPlans: (...args: unknown[]) => mockListPlans(...args),
    getSubscription: (...args: unknown[]) => mockGetSubscription(...args),
    subscribe: (...args: unknown[]) => mockSubscribe(...args),
    listPayments: (...args: unknown[]) => mockListPayments(...args),
  };
});

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

const plansFixture = [
  {
    id: 'plan-1',
    name: 'Basic Plan',
    description: 'Entry level mailbox',
    price_cedis: 19.99,
    interval: 'monthly' as const,
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
    interval: 'monthly' as const,
    max_mailboxes: 10,
    storage_gb: 50,
    features: { custom_domain: true },
    active: true,
    created_at: '2026-04-15T00:00:00Z',
    updated_at: '2026-04-15T00:00:00Z',
  },
];

describe('BillingManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListPlans.mockResolvedValue(plansFixture);
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

  it('renders Pay with Card, Mastercard, Invoice, and Bank Transfer buttons per plan', async () => {
    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getAllByText('Pay with Card')).toHaveLength(2);
      expect(screen.getAllByText('Mastercard')).toHaveLength(2);
      expect(screen.getAllByText('Invoice')).toHaveLength(2);
      expect(screen.getAllByText('Bank Transfer')).toHaveLength(2);
    });
  });

  it('redirects to Paystack authorization URL when paystack returns a hosted checkout URL', async () => {
    mockSubscribe.mockResolvedValue({
      subscription_id: 'sub-1',
      payment_id: 'pay-1',
      provider: 'paystack',
      authorization_url: 'https://checkout.paystack.com/abc123',
      reference: 'TMAIL-abc123',
    });
    // Use vi.stubGlobal to swap window.location with a writable href stub for the duration of this test.
    const locStub = { href: '' };
    vi.stubGlobal('location', locStub);

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByTestId('pay-paystack-plan-1')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('pay-paystack-plan-1'));

    await waitFor(() => {
      expect(mockSubscribe).toHaveBeenCalledWith({ plan_id: 'plan-1', provider: 'paystack' });
      expect(locStub.href).toBe('https://checkout.paystack.com/abc123');
    });

    vi.unstubAllGlobals();
  });

  it('shows inline instructions for bank transfer (no redirect)', async () => {
    mockSubscribe.mockResolvedValue({
      subscription_id: 'sub-2',
      payment_id: 'pay-2',
      provider: 'bank_transfer',
      authorization_url:
        'bank_transfer:Pay GHS 19.99 to Acme Bank, Acct 0123456789, Ref TMAIL-xyz',
      reference: 'TMAIL-xyz',
    });

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByTestId('pay-bank-plan-1')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('pay-bank-plan-1'));

    await waitFor(() => {
      expect(mockSubscribe).toHaveBeenCalledWith({ plan_id: 'plan-1', provider: 'bank_transfer' });
      expect(screen.getByTestId('payment-instructions')).toBeInTheDocument();
      expect(screen.getByTestId('payment-instructions-detail').textContent).toContain('Acme Bank');
      expect(screen.getByTestId('payment-instructions-detail').textContent).toContain('TMAIL-xyz');
    });
  });

  it('shows inline instructions for mastercard mpgs session (no redirect)', async () => {
    mockSubscribe.mockResolvedValue({
      subscription_id: 'sub-3',
      payment_id: 'pay-3',
      provider: 'mastercard',
      authorization_url: 'mpgs:session:SESSION0001',
      reference: 'TMAIL-mc1',
    });

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByTestId('pay-mastercard-plan-2')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByTestId('pay-mastercard-plan-2'));

    await waitFor(() => {
      expect(mockSubscribe).toHaveBeenCalledWith({ plan_id: 'plan-2', provider: 'mastercard' });
      const panel = screen.getByTestId('payment-instructions');
      expect(panel).toBeInTheDocument();
      expect(panel.textContent).toContain('Mastercard');
      expect(panel.textContent).toContain('TMAIL-mc1');
    });
  });

  it('renders payment history with provider labels for every backend-supported provider', async () => {
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
      {
        id: 'pay-2',
        user_id: 'user-1',
        subscription_id: 'sub-2',
        provider: 'mastercard',
        provider_ref: 'TMAIL-mc1',
        amount_cedis: 49.99,
        currency: 'GHS',
        status: 'pending',
        metadata: {},
        created_at: '2026-04-16T12:00:00Z',
      },
      {
        id: 'pay-3',
        user_id: 'user-1',
        subscription_id: 'sub-3',
        provider: 'cybersource',
        provider_ref: 'TMAIL-cs1',
        amount_cedis: 49.99,
        currency: 'GHS',
        status: 'failed',
        metadata: {},
        created_at: '2026-04-17T12:00:00Z',
      },
      {
        id: 'pay-4',
        user_id: 'user-1',
        subscription_id: 'sub-4',
        provider: 'bank_transfer',
        provider_ref: 'TMAIL-bt1',
        amount_cedis: 19.99,
        currency: 'GHS',
        status: 'pending',
        metadata: {},
        created_at: '2026-04-18T12:00:00Z',
      },
    ]);

    render(<BillingManager />, { wrapper });
    await waitFor(() => {
      expect(screen.getByText('TMAIL-abc123')).toBeInTheDocument();
      expect(screen.getByText('TMAIL-mc1')).toBeInTheDocument();
      expect(screen.getByText('TMAIL-cs1')).toBeInTheDocument();
      expect(screen.getByText('TMAIL-bt1')).toBeInTheDocument();
      expect(screen.getAllByText('Paystack').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('Mastercard').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('Cybersource').length).toBeGreaterThanOrEqual(1);
      expect(screen.getAllByText('Bank Transfer').length).toBeGreaterThanOrEqual(1);
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
    const backBtn = screen
      .getByText('Billing & Subscription')
      .closest('.settings-panel__header')
      ?.querySelector('.btn--ghost');
    if (backBtn) {
      fireEvent.click(backBtn);
      expect(mockSetViewMode).toHaveBeenCalledWith('list');
    }
  });
});
