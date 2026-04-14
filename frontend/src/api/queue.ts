// Added: Email queue API client for TMAIL-58
import { apiClient } from './client';

/// PURPOSE: Represents a single queued email with retry tracking
export interface EmailQueueItem {
  id: string;
  mailbox_id: string;
  to_addresses: string[];
  cc_addresses: string[];
  bcc_addresses: string[];
  subject: string;
  body_html: string;
  body_text: string;
  status: 'pending' | 'sending' | 'sent' | 'failed' | 'dead_letter';
  retry_count: number;
  max_retries: number;
  next_retry_at: string;
  last_error: string | null;
  created_at: string;
  sent_at: string | null;
  failed_at: string | null;
}

/// PURPOSE: Aggregated queue counts by status
export interface QueueStats {
  pending: number;
  sending: number;
  sent: number;
  failed: number;
  dead_letter: number;
}

/// PURPOSE: Fetch queued emails for current user, optionally filtered by status
export async function fetchQueueItems(status?: string): Promise<EmailQueueItem[]> {
  const query = status ? `?status=${encodeURIComponent(status)}` : '';
  return apiClient.get<EmailQueueItem[]>(`/queue${query}`);
}

/// PURPOSE: Fetch queue statistics (counts by status)
export async function fetchQueueStats(): Promise<QueueStats> {
  return apiClient.get<QueueStats>('/queue/stats');
}

/// PURPOSE: Cancel/remove a queued email that is not currently sending
export async function cancelQueueItem(id: string): Promise<void> {
  await apiClient.delete(`/queue/${id}`);
}

/// PURPOSE: Retry a failed or dead_letter email (resets to pending)
export async function retryQueueItem(id: string): Promise<void> {
  await apiClient.post(`/queue/${id}/retry`);
}
