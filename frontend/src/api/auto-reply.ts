import { apiClient } from './client';

export interface AutoReplyRule {
  id: string;
  mailbox_id: string;
  enabled: boolean;
  subject: string;
  body_text: string;
  body_html: string | null;
  start_date: string | null;
  end_date: string | null;
  reply_to_all: boolean;
  exclude_lists: boolean;
  created_at: string;
  updated_at: string;
}

export interface UpsertAutoReply {
  enabled: boolean;
  subject: string;
  body_text: string;
  body_html?: string;
  start_date?: string;
  end_date?: string;
  reply_to_all?: boolean;
  exclude_lists?: boolean;
}

export const autoReplyApi = {
  get: () => apiClient.get<AutoReplyRule | null>('/auto-reply'),
  set: (data: UpsertAutoReply) => apiClient.put<AutoReplyRule>('/auto-reply', data),
};
