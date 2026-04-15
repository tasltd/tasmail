// Added: Rspamd spam filter API client for TMAIL-15
import { apiClient } from './client';

/// PURPOSE: Domain-level spam filter configuration
export interface SpamSettings {
  id: string;
  domain_id: string | null;
  threshold_reject: number;
  threshold_greylist: number;
  threshold_add_header: number;
  learn_spam_enabled: boolean;
  learn_ham_enabled: boolean;
  dkim_signing_enabled: boolean;
  arc_signing_enabled: boolean;
  autolearn_enabled: boolean;
  custom_rules: unknown[];
  created_at: string;
  updated_at: string;
}

/// PURPOSE: Request body for updating spam settings
export interface UpdateSpamSettings {
  threshold_reject?: number;
  threshold_greylist?: number;
  threshold_add_header?: number;
  learn_spam_enabled?: boolean;
  learn_ham_enabled?: boolean;
  dkim_signing_enabled?: boolean;
  arc_signing_enabled?: boolean;
  autolearn_enabled?: boolean;
  custom_rules?: unknown[];
}

/// PURPOSE: Quarantined email record with spam scoring details
export interface SpamQuarantineItem {
  id: string;
  user_id: string;
  message_id: string;
  sender: string | null;
  subject: string | null;
  score: number;
  action: 'reject' | 'greylist' | 'add_header' | 'no_action';
  symbols: unknown[];
  quarantined_at: string;
  released: boolean;
  released_at: string | null;
}

/// PURPOSE: Aggregated spam statistics
export interface SpamStats {
  total_scanned: number;
  total_blocked: number;
  total_passed: number;
  quarantined: number;
  released: number;
}

/// PURPOSE: Request body for learning a message as spam or ham
export interface LearnRequest {
  message_id: string;
  folder: string;
  is_spam: boolean;
}

/// PURPOSE: Fetch spam settings for the current user's domain
export async function fetchSpamSettings(): Promise<SpamSettings | null> {
  return apiClient.get<SpamSettings | null>('/spam/settings');
}

/// PURPOSE: Update spam thresholds and toggles (admin only)
export async function updateSpamSettings(data: UpdateSpamSettings): Promise<SpamSettings> {
  return apiClient.put<SpamSettings>('/spam/settings', data);
}

/// PURPOSE: Fetch quarantined messages for current user
export async function fetchQuarantine(): Promise<SpamQuarantineItem[]> {
  return apiClient.get<SpamQuarantineItem[]>('/spam/quarantine');
}

/// PURPOSE: Release a message from quarantine back to inbox
export async function releaseQuarantine(id: string): Promise<void> {
  await apiClient.post(`/spam/quarantine/${id}/release`);
}

/// PURPOSE: Permanently delete a quarantined message
export async function deleteQuarantine(id: string): Promise<void> {
  await apiClient.delete(`/spam/quarantine/${id}`);
}

/// PURPOSE: Learn a message as spam or ham via Rspamd
export async function learnMessage(data: LearnRequest): Promise<void> {
  await apiClient.post('/spam/learn', data);
}

/// PURPOSE: Fetch spam statistics
export async function fetchSpamStats(): Promise<SpamStats> {
  return apiClient.get<SpamStats>('/spam/stats');
}
