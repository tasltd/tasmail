// Added: DANE API client for DNS-based Authentication of Named Entities (TMAIL-125)

import { apiClient } from './client';

// Added: DANE policy interface matching backend DanePolicy struct
export interface DanePolicy {
  id: string;
  domain: string;
  enforce: boolean;
  last_checked_at: string | null;
  tlsa_records: TlsaRecord[];
  created_at: string;
  updated_at: string;
}

// Added: TLSA record parsed from DNS
export interface TlsaRecord {
  usage: number;
  selector: number;
  matching_type: number;
  cert_data: string;
}

// Added: DANE verification result for an outbound message
export interface DaneVerification {
  id: string;
  user_id: string;
  message_id: string;
  recipient_domain: string;
  dane_status: 'verified' | 'failed' | 'no_tlsa' | 'disabled';
  checked_at: string;
}

// Added: Result of a DANE/TLSA lookup
export interface DaneResult {
  domain: string;
  status: string;
  tlsa_records: TlsaRecord[];
  message: string;
}

// Added: Request body for creating/updating a DANE policy
export interface CreateDanePolicyRequest {
  domain: string;
  enforce?: boolean;
}

// Added: Request body for TLSA record lookup
export interface DaneLookupRequest {
  domain: string;
  port?: number;
}

// PURPOSE: List all DANE policies (admin)
export async function listDanePolicies(): Promise<DanePolicy[]> {
  return apiClient.get<DanePolicy[]>('/admin/dane');
}

// PURPOSE: Create or update a DANE policy for a domain (admin)
export async function createDanePolicy(data: CreateDanePolicyRequest): Promise<DanePolicy> {
  return apiClient.post<DanePolicy>('/admin/dane', data);
}

// PURPOSE: Delete a DANE policy by ID (admin)
export async function deleteDanePolicy(id: string): Promise<void> {
  await apiClient.delete(`/admin/dane/${id}`);
}

// PURPOSE: Lookup TLSA records for a domain (admin diagnostic tool)
export async function lookupTlsa(data: DaneLookupRequest): Promise<DaneResult> {
  return apiClient.post<DaneResult>('/admin/dane/lookup', data);
}

// PURPOSE: List DANE verifications for the current user's sent messages
export async function listDaneVerifications(limit?: number, offset?: number): Promise<DaneVerification[]> {
  const params = new URLSearchParams();
  if (limit !== undefined) params.set('limit', String(limit));
  if (offset !== undefined) params.set('offset', String(offset));
  const query = params.toString();
  return apiClient.get<DaneVerification[]>(`/dane/verifications${query ? `?${query}` : ''}`);
}
