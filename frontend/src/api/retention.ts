// Added: Retention policy and legal hold API client for TMAIL-109

import { apiClient } from './client';

// Added: Retention policy interface matching backend model
export interface RetentionPolicy {
  id: string;
  name: string;
  description: string | null;
  retention_days: number;
  folder_pattern: string | null;
  apply_to_all: boolean;
  created_at: string;
  updated_at: string;
}

// Added: Legal hold interface matching backend model
export interface LegalHold {
  id: string;
  user_id: string;
  reason: string;
  placed_by: string;
  active: boolean;
  created_at: string;
  released_at: string | null;
}

export interface CreateRetentionPolicyRequest {
  name: string;
  description?: string;
  retention_days: number;
  folder_pattern?: string;
  apply_to_all?: boolean;
}

export interface UpdateRetentionPolicyRequest {
  name?: string;
  description?: string;
  retention_days?: number;
  folder_pattern?: string;
  apply_to_all?: boolean;
}

export interface CreateLegalHoldRequest {
  user_id: string;
  reason: string;
}

// PURPOSE: List all retention policies (admin only)
export async function listRetentionPolicies(): Promise<RetentionPolicy[]> {
  return apiClient.get<RetentionPolicy[]>('/admin/retention');
}

// PURPOSE: Create a new retention policy (admin only)
export async function createRetentionPolicy(data: CreateRetentionPolicyRequest): Promise<RetentionPolicy> {
  return apiClient.post<RetentionPolicy>('/admin/retention', data);
}

// PURPOSE: Update an existing retention policy (admin only)
export async function updateRetentionPolicy(id: string, data: UpdateRetentionPolicyRequest): Promise<RetentionPolicy> {
  return apiClient.put<RetentionPolicy>(`/admin/retention/${id}`, data);
}

// PURPOSE: Delete a retention policy (admin only)
export async function deleteRetentionPolicy(id: string): Promise<void> {
  await apiClient.delete(`/admin/retention/${id}`);
}

// PURPOSE: List all legal holds (admin only)
export async function listLegalHolds(): Promise<LegalHold[]> {
  return apiClient.get<LegalHold[]>('/admin/legal-holds');
}

// PURPOSE: Place a legal hold on a user (admin only)
export async function createLegalHold(data: CreateLegalHoldRequest): Promise<LegalHold> {
  return apiClient.post<LegalHold>('/admin/legal-holds', data);
}

// PURPOSE: Release a legal hold (admin only)
export async function releaseLegalHold(id: string): Promise<LegalHold> {
  return apiClient.put<LegalHold>(`/admin/legal-holds/${id}/release`, {});
}
