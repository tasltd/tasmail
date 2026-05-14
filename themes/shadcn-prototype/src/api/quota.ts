// TMAIL-232: ported quota client so the alt-UI Admin dashboard can show
// real storage usage instead of mocked totals.
import { apiClient } from './client';

export interface QuotaStatus {
  mailbox_id: string;
  quota_bytes: number;
  used_bytes: number;
  message_count: number;
  usage_percent: number;
  quota_warn_percent: number;
  is_over_quota: boolean;
  is_warning: boolean;
  last_synced_at: string | null;
}

export const quotaApi = {
  getQuota: () => apiClient.get<QuotaStatus>('/quota'),
  syncQuota: () => apiClient.post<QuotaStatus>('/quota/sync'),
};
