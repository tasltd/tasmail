// TMAIL-201: admin client for payment_provider_config CRUD.
import { apiClient } from './client';

export type PaymentProviderType = 'PAYSTACK' | 'MASTERCARD' | 'CYBERSOURCE' | 'BANK_TRANSFER';

export interface ProviderSummary {
  id: string;
  provider: PaymentProviderType;
  tenant_id: string | null;
  name: string | null;
  description: string | null;
  base_url: string | null;
  callback_url: string | null;
  currency: string | null;
  environment: string | null;
  enabled: boolean;
  archived: boolean;
  has_secret_key: boolean;
  has_public_key: boolean;
  has_webhook_secret: boolean;
  has_merchant_id: boolean;
  has_api_password: boolean;
  has_key_id: boolean;
  has_shared_secret_key: boolean;
  bank_details: Record<string, unknown> | null;
  split_code: string | null;
}

export interface UpsertProviderRequest {
  provider: PaymentProviderType;
  tenant_id?: string;
  name?: string;
  description?: string;
  secret_key?: string;
  public_key?: string;
  webhook_secret?: string;
  merchant_id?: string;
  api_password?: string;
  key_id?: string;
  shared_secret_key?: string;
  key_file_path?: string;
  base_url?: string;
  callback_url?: string;
  currency?: string;
  environment?: string;
  bank_details?: Record<string, unknown>;
  split_code?: string;
  notes?: string;
}

export const adminPaymentProvidersApi = {
  list: () => apiClient.get<ProviderSummary[]>('/admin/payment-providers'),
  create: (body: UpsertProviderRequest) => apiClient.post<ProviderSummary>('/admin/payment-providers', body),
  archive: (id: string) => apiClient.delete<void>(`/admin/payment-providers/${id}`),
};
