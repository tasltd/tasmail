// Added: SMTP configuration API client for BYO-SMTP (TMAIL-48)

import { apiClient } from './client';

// Added: SMTP encryption types matching backend enum
export type SmtpEncryption = 'none' | 'ssl' | 'starttls';

export interface SmtpConfiguration {
  id: string;
  user_id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  password_masked: string;
  encryption: string;
  from_address: string | null;
  is_default: boolean;
  verified: boolean;
  last_tested_at: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface CreateSmtpConfigRequest {
  name: string;
  host: string;
  port?: number;
  username: string;
  password: string;
  encryption?: SmtpEncryption;
  from_address?: string;
}

export interface UpdateSmtpConfigRequest {
  name?: string;
  host?: string;
  port?: number;
  username?: string;
  password?: string;
  encryption?: SmtpEncryption;
  from_address?: string;
}

export interface SmtpTestResult {
  success: boolean;
  message: string;
  latency_ms: number;
}

// PURPOSE: List all SMTP configs for the current user (passwords masked)
export async function listSmtpConfigs(): Promise<SmtpConfiguration[]> {
  return apiClient.get<SmtpConfiguration[]>('/smtp-configs');
}

// PURPOSE: Create a new SMTP configuration
export async function createSmtpConfig(data: CreateSmtpConfigRequest): Promise<SmtpConfiguration> {
  return apiClient.post<SmtpConfiguration>('/smtp-configs', data);
}

// PURPOSE: Get a single SMTP configuration by ID
export async function getSmtpConfig(id: string): Promise<SmtpConfiguration> {
  return apiClient.get<SmtpConfiguration>(`/smtp-configs/${id}`);
}

// PURPOSE: Update an existing SMTP configuration
export async function updateSmtpConfig(id: string, data: UpdateSmtpConfigRequest): Promise<SmtpConfiguration> {
  return apiClient.put<SmtpConfiguration>(`/smtp-configs/${id}`, data);
}

// PURPOSE: Delete an SMTP configuration
export async function deleteSmtpConfig(id: string): Promise<void> {
  await apiClient.delete(`/smtp-configs/${id}`);
}

// PURPOSE: Test an SMTP configuration by connecting and sending a test email
export async function testSmtpConfig(id: string): Promise<SmtpTestResult> {
  return apiClient.post<SmtpTestResult>(`/smtp-configs/${id}/test`, {});
}

// PURPOSE: Set an SMTP configuration as the default for sending
export async function setDefaultSmtp(id: string): Promise<SmtpConfiguration> {
  return apiClient.post<SmtpConfiguration>(`/smtp-configs/${id}/default`, {});
}
