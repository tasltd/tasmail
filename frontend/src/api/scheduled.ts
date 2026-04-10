import { apiClient } from './client';

export interface ScheduleResponse {
  id: string;
  cancel_token: string;
  scheduled_at: string;
  can_undo_until: string;
}

export interface ScheduledEmail {
  id: string;
  mailbox_id: string;
  to_addresses: string[];
  cc_addresses: string[];
  bcc_addresses: string[];
  subject: string;
  text_body: string | null;
  html_body: string | null;
  scheduled_at: string;
  status: string;
  cancel_token: string;
  created_at: string;
  sent_at: string | null;
  cancelled_at: string | null;
}

export interface ScheduleSendRequest {
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  text_body?: string;
  html_body?: string;
  scheduled_at?: string;
  delay_seconds?: number;
}

export const scheduledApi = {
  scheduleSend: (req: ScheduleSendRequest) =>
    apiClient.post<ScheduleResponse>('/messages/schedule', req),
  cancelScheduled: (cancelToken: string) =>
    apiClient.post<void>(`/messages/cancel/${cancelToken}`),
  listScheduled: (status?: string) =>
    apiClient.get<ScheduledEmail[]>(`/messages/scheduled${status ? `?status=${status}` : ''}`),
};
