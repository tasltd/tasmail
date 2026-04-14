// Added: POP3 configuration API client for Dovecot POP3 access (TMAIL-133)

import { apiClient } from './client';

// Added: POP3 configuration types matching backend structs
export interface Pop3Configuration {
  id: string;
  user_id: string;
  enabled: boolean;
  delete_after_download: boolean;
  retention_days: number | null;
  last_pop3_login: string | null;
  created_at: string | null;
  updated_at: string | null;
}

export interface UpdatePop3ConfigRequest {
  enabled?: boolean;
  delete_after_download?: boolean;
  retention_days?: number | null;
}

export interface Pop3Status {
  server: string;
  port: number;
  encryption: string;
  username_format: string;
}

// PURPOSE: Get current user's POP3 configuration
export async function getPop3Config(): Promise<Pop3Configuration | null> {
  return apiClient.get<Pop3Configuration | null>('/pop3/config');
}

// PURPOSE: Create or update POP3 configuration
export async function updatePop3Config(data: UpdatePop3ConfigRequest): Promise<Pop3Configuration> {
  return apiClient.put<Pop3Configuration>('/pop3/config', data);
}

// PURPOSE: Delete POP3 configuration (disable POP3 access)
export async function deletePop3Config(): Promise<void> {
  await apiClient.delete('/pop3/config');
}

// PURPOSE: Get POP3 connection info for mail client setup
export async function getPop3Status(): Promise<Pop3Status> {
  return apiClient.get<Pop3Status>('/pop3/status');
}
