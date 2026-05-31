// Added (TMAIL-346): BYOK API client for the Modern UI's onboarding wizard.
// Mirrors frontend/src/api/byok.ts so the Modern UI can attach an IMAP/SMTP
// server without bouncing back to the classic SPA.
import { apiClient } from './client';

export type Encryption = 'ssl' | 'starttls' | 'none';

export interface ProviderPreset {
  name: string;
  domain: string;
  imap: { host: string; port: number; encryption: Encryption };
  smtp: { host: string; port: number; encryption: Encryption };
  hint?: string;
}

export interface ImapConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  encryption: Encryption;
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
  encryption: Encryption;
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
  encryption: Encryption;
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
  encryption: Encryption;
  from_address?: string;
  is_default?: boolean;
}

export interface TestResult {
  ok: boolean;
  message: string;
}

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
