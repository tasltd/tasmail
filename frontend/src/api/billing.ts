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
