// Added: Billing API client for Paystack/MoMo payment integration (TMAIL-46)

import { apiClient } from './client';

// Added: Billing plan type matching backend BillingPlan struct
export interface BillingPlan {
  id: string;
  name: string;
  description: string | null;
  price_cedis: number;
  interval: 'monthly' | 'yearly';
  max_mailboxes: number;
  storage_gb: number;
  features: Record<string, unknown>;
  active: boolean | null;
  created_at: string | null;
  updated_at: string | null;
}

// Added: Subscription type matching backend Subscription struct
export interface Subscription {
  id: string;
  user_id: string;
  plan_id: string;
  provider: 'paystack' | 'mtn_momo';
  provider_subscription_id: string | null;
  status: string;
  current_period_start: string | null;
  current_period_end: string | null;
  cancelled_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

// Added: Payment record type matching backend Payment struct
export interface Payment {
  id: string;
  user_id: string;
  subscription_id: string | null;
  provider: 'paystack' | 'mtn_momo';
  provider_ref: string;
  amount_cedis: number;
  currency: string | null;
  status: 'pending' | 'success' | 'failed' | 'refunded';
  metadata: Record<string, unknown>;
  created_at: string | null;
}

// Added: Subscribe request body
export interface SubscribeRequest {
  plan_id: string;
  provider: 'paystack' | 'mtn_momo';
  phone_number?: string;
}

// Added: Subscribe response from backend
export interface SubscribeResponse {
  subscription_id: string;
  payment_id: string;
  provider: string;
  authorization_url: string | null;
  reference: string;
}

// PURPOSE: List all active billing plans (public — no auth needed)
export async function listPlans(): Promise<BillingPlan[]> {
  return apiClient.get<BillingPlan[]>('/billing/plans');
}

// PURPOSE: Get current user's active subscription
export async function getSubscription(): Promise<Subscription | null> {
  return apiClient.get<Subscription | null>('/billing/subscription');
}

// PURPOSE: Initialize a new subscription with Paystack or MoMo
export async function subscribe(data: SubscribeRequest): Promise<SubscribeResponse> {
  return apiClient.post<SubscribeResponse>('/billing/subscribe', data);
}

// PURPOSE: List current user's payment history
export async function listPayments(): Promise<Payment[]> {
  return apiClient.get<Payment[]>('/billing/payments');
}

// =====================================================================
// TMAIL-178/179 — usage-based billing
// =====================================================================

export interface UsageResponse {
  period_start: string;          // YYYY-MM-DD
  period_end: string;            // YYYY-MM-DD
  avg_storage_bytes: number;
  peak_storage_bytes: number;
  current_storage_bytes: number;
  sample_count: number;
  projected_amount_ghs: number;
  projected_minimum_applied: boolean;
  projected_billed_gb: number;
  ghs_per_gb: number;
  ghs_monthly_min: number;
}

export interface UsageInvoiceRow {
  id: string;
  period_start: string;
  period_end: string;
  avg_storage_bytes: number;
  amount_ghs: number;
  minimum_applied: boolean;
  status: 'pending' | 'paid' | 'failed' | 'waived';
  provider: string | null;
  provider_reference: string | null;
  paid_at: string | null;
  created_at: string | null;
}

export const usageBillingApi = {
  usage: () => apiClient.get<UsageResponse>('/billing/usage'),
  invoices: () => apiClient.get<UsageInvoiceRow[]>('/billing/invoices'),
};
