// Added: BYOK API client — IMAP + SMTP per-user configuration for the onboarding wizard.
import { apiClient } from './client';

export interface ProviderPreset {
  name: string;
  domain: string;
  imap: { host: string; port: number; encryption: 'ssl' | 'starttls' | 'none' };
  smtp: { host: string; port: number; encryption: 'ssl' | 'starttls' | 'none' };
  hint?: string;
}

export interface ImapConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  encryption: 'ssl' | 'starttls' | 'none';
  is_default: boolean;
  verified: boolean;
  last_tested_at: string | null;
  last_error: string | null;
}

export interface CreateImapConfig {
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  encryption: 'ssl' | 'starttls' | 'none';
  is_default?: boolean;
  sent_folder?: string;
  drafts_folder?: string;
  trash_folder?: string;
  spam_folder?: string;
  archive_folder?: string;
}

export interface SmtpConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  encryption: 'ssl' | 'starttls' | 'none';
  is_default: boolean;
  from_address?: string | null;
  verified: boolean;
}

export interface CreateSmtpConfig {
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  encryption: 'ssl' | 'starttls' | 'none';
  from_address?: string;
  is_default?: boolean;
}

export interface TestResult { ok: boolean; message: string; }

export const byokApi = {
  presets: () => apiClient.get<ProviderPreset[]>('/imap-configs/presets'),
  listImap: () => apiClient.get<ImapConfig[]>('/imap-configs'),
  createImap: (req: CreateImapConfig) => apiClient.post<ImapConfig>('/imap-configs', req),
  deleteImap: (id: string) => apiClient.delete(`/imap-configs/${id}`),
  testImap: (req: Omit<CreateImapConfig, 'name' | 'is_default'>) =>
    apiClient.post<TestResult>('/imap-configs/test', req),

  listSmtp: () => apiClient.get<SmtpConfig[]>('/smtp-configs'),
  createSmtp: (req: CreateSmtpConfig) => apiClient.post<SmtpConfig>('/smtp-configs', req),
  deleteSmtp: (id: string) => apiClient.delete(`/smtp-configs/${id}`),
};
