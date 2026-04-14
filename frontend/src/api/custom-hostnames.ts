// Added: Custom hostname API client for per-tenant SNI management (TMAIL-112)

import { apiClient } from './client';

// Added: Custom hostname interface matching backend CustomHostname struct
export interface CustomHostname {
  id: string;
  domain_id: string;
  smtp_hostname: string;
  imap_hostname: string;
  webmail_hostname: string | null;
  autodiscover_hostname: string | null;
  tls_cert_path: string | null;
  tls_key_path: string | null;
  verified: boolean;
  verified_at: string | null;
  dns_verification_token: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateHostnameRequest {
  domain_id: string;
  smtp_hostname: string;
  imap_hostname: string;
  webmail_hostname?: string;
  autodiscover_hostname?: string;
  tls_cert_path?: string;
  tls_key_path?: string;
}

export interface UpdateHostnameRequest {
  smtp_hostname?: string;
  imap_hostname?: string;
  webmail_hostname?: string;
  autodiscover_hostname?: string;
  tls_cert_path?: string;
  tls_key_path?: string;
}

// PURPOSE: List all custom hostname configurations (admin only)
export async function listHostnames(): Promise<CustomHostname[]> {
  return apiClient.get<CustomHostname[]>('/admin/hostnames');
}

// PURPOSE: Create a new custom hostname config for a domain
export async function createHostname(data: CreateHostnameRequest): Promise<CustomHostname> {
  return apiClient.post<CustomHostname>('/admin/hostnames', data);
}

// PURPOSE: Get a single custom hostname config by ID
export async function getHostname(id: string): Promise<CustomHostname> {
  return apiClient.get<CustomHostname>(`/admin/hostnames/${id}`);
}

// PURPOSE: Update an existing custom hostname config
export async function updateHostname(id: string, data: UpdateHostnameRequest): Promise<CustomHostname> {
  return apiClient.put<CustomHostname>(`/admin/hostnames/${id}`, data);
}

// PURPOSE: Delete a custom hostname configuration
export async function deleteHostname(id: string): Promise<void> {
  await apiClient.delete(`/admin/hostnames/${id}`);
}

// PURPOSE: Trigger DNS verification for a custom hostname config
export async function verifyHostname(id: string): Promise<CustomHostname> {
  return apiClient.post<CustomHostname>(`/admin/hostnames/${id}/verify`, {});
}
