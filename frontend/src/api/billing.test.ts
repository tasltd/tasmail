// Added: Billing API client tests for TMAIL-46

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listPlans, getSubscription, subscribe, listPayments } from './billing';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('billing API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listPlans', () => {
    it('calls GET /billing/plans', async () => {
      const mockPlans = [
        {
          id: 'plan-1',
          name: 'Basic',
          description: 'Basic plan with 1 mailbox',
          price_cedis: 19.99,
          interval: 'monthly',
          max_mailboxes: 1,
          storage_gb: 5,
          features: { custom_domain: false },
          active: true,
          created_at: '2026-04-15T00:00:00Z',
          updated_at: '2026-04-15T00:00:00Z',
        },
        {
          id: 'plan-2',
          name: 'Pro',
          description: 'Pro plan with 10 mailboxes',
          price_cedis: 49.99,
          interval: 'monthly',
          max_mailboxes: 10,
          storage_gb: 50,
          features: { custom_domain: true },
          active: true,
          created_at: '2026-04-15T00:00:00Z',
          updated_at: '2026-04-15T00:00:00Z',
        },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockPlans);

      const result = await listPlans();
      expect(apiClient.get).toHaveBeenCalledWith('/billing/plans');
      expect(result).toHaveLength(2);
      expect(result[0].name).toBe('Basic');
      expect(result[1].price_cedis).toBe(49.99);
    });

    it('returns empty array when no plans', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);

      const result = await listPlans();
      expect(result).toEqual([]);
    });
  });

  describe('getSubscription', () => {
    it('calls GET /billing/subscription and returns subscription', async () => {
      const mockSub = {
        id: 'sub-1',
        user_id: 'user-1',
        plan_id: 'plan-1',
        provider: 'paystack',
        provider_subscription_id: null,
        status: 'active',
        current_period_start: '2026-04-01T00:00:00Z',
        current_period_end: '2026-05-01T00:00:00Z',
        cancelled_at: null,
        created_at: '2026-04-01T00:00:00Z',
        updated_at: '2026-04-01T00:00:00Z',
      };
      vi.mocked(apiClient.get).mockResolvedValue(mockSub);

      const result = await getSubscription();
      expect(apiClient.get).toHaveBeenCalledWith('/billing/subscription');
      expect(result?.status).toBe('active');
    });

    it('returns null when no active subscription', async () => {
      vi.mocked(apiClient.get).mockResolvedValue(null);

      const result = await getSubscription();
      expect(result).toBeNull();
    });
  });

  describe('subscribe', () => {
    it('calls POST /billing/subscribe with Paystack provider', async () => {
      const reqData = {
        plan_id: 'plan-1',
        provider: 'paystack' as const,
      };
      const mockResp = {
        subscription_id: 'sub-1',
        payment_id: 'pay-1',
        provider: 'paystack',
        authorization_url: 'https://checkout.paystack.com/abc123',
        reference: 'TMAIL-abc123',
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockResp);

      const result = await subscribe(reqData);
      expect(apiClient.post).toHaveBeenCalledWith('/billing/subscribe', reqData);
      expect(result.authorization_url).toContain('paystack.com');
    });

    it('calls POST /billing/subscribe with MoMo provider and phone', async () => {
      const reqData = {
        plan_id: 'plan-2',
        provider: 'mtn_momo' as const,
        phone_number: '0241234567',
      };
      const mockResp = {
        subscription_id: 'sub-2',
        payment_id: 'pay-2',
        provider: 'mtn_momo',
        authorization_url: null,
        reference: 'TMAIL-def456',
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockResp);

      const result = await subscribe(reqData);
      expect(apiClient.post).toHaveBeenCalledWith('/billing/subscribe', reqData);
      expect(result.authorization_url).toBeNull();
      expect(result.reference).toBe('TMAIL-def456');
    });
  });

  describe('listPayments', () => {
    it('calls GET /billing/payments', async () => {
      const mockPayments = [
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
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockPayments);

      const result = await listPayments();
      expect(apiClient.get).toHaveBeenCalledWith('/billing/payments');
      expect(result).toHaveLength(1);
      expect(result[0].amount_cedis).toBe(19.99);
      expect(result[0].status).toBe('success');
    });

    it('returns empty array when no payments', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);

      const result = await listPayments();
      expect(result).toEqual([]);
    });
  });
});
