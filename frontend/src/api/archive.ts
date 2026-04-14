// Added: Archive API client for Piler email archiving integration (TMAIL-107)

import { apiClient } from './client';

// Added: Archive policy type matching backend archive_policies table
export interface ArchivePolicy {
  id: string;
  name: string;
  description: string | null;
  match_criteria: Record<string, unknown>;
  archive_after_days: number;
  delete_original: boolean;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

// Added: Archive server configuration type matching backend archive_config table
export interface ArchiveConfig {
  id: string;
  piler_url: string | null;
  piler_api_key_encrypted: string | null;
  retention_years: number;
  enabled: boolean;
  updated_at: string;
}

// Added: Archive search result from Piler or mock
export interface ArchiveSearchResult {
  id: string;
  subject: string;
  sender: string;
  recipients: string[];
  date: string;
  size: number;
  has_attachment: boolean;
}

// Added: Archive search history entry for audit trail
export interface ArchiveSearchHistoryEntry {
  id: string;
  user_id: string;
  query: string;
  filters: Record<string, unknown> | null;
  result_count: number | null;
  searched_at: string;
}

export interface CreateArchivePolicyRequest {
  name: string;
  description?: string;
  match_criteria?: Record<string, unknown>;
  archive_after_days?: number;
  delete_original?: boolean;
  enabled?: boolean;
}

export interface UpdateArchivePolicyRequest {
  name?: string;
  description?: string;
  match_criteria?: Record<string, unknown>;
  archive_after_days?: number;
  delete_original?: boolean;
  enabled?: boolean;
}

export interface UpdateArchiveConfigRequest {
  piler_url?: string;
  piler_api_key?: string;
  retention_years?: number;
  enabled?: boolean;
}

export interface ArchiveSearchRequest {
  query: string;
  date_from?: string;
  date_to?: string;
  sender?: string;
  recipient?: string;
}

// PURPOSE: List all archive policies (admin endpoint)
export async function listArchivePolicies(): Promise<ArchivePolicy[]> {
  return apiClient.get<ArchivePolicy[]>('/admin/archive/policies');
}

// PURPOSE: Create a new archive policy
export async function createArchivePolicy(data: CreateArchivePolicyRequest): Promise<ArchivePolicy> {
  return apiClient.post<ArchivePolicy>('/admin/archive/policies', data);
}

// PURPOSE: Update an existing archive policy by ID
export async function updateArchivePolicy(id: string, data: UpdateArchivePolicyRequest): Promise<ArchivePolicy> {
  return apiClient.put<ArchivePolicy>(`/admin/archive/policies/${id}`, data);
}

// PURPOSE: Delete an archive policy
export async function deleteArchivePolicy(id: string): Promise<void> {
  await apiClient.delete(`/admin/archive/policies/${id}`);
}

// PURPOSE: Get archive server configuration
export async function getArchiveConfig(): Promise<ArchiveConfig | null> {
  return apiClient.get<ArchiveConfig | null>('/admin/archive/config');
}

// PURPOSE: Update archive server configuration (Piler URL, API key, etc.)
export async function updateArchiveConfig(data: UpdateArchiveConfigRequest): Promise<ArchiveConfig> {
  return apiClient.put<ArchiveConfig>('/admin/archive/config', data);
}

// PURPOSE: Search archived emails via Piler integration
export async function searchArchive(data: ArchiveSearchRequest): Promise<ArchiveSearchResult[]> {
  return apiClient.post<ArchiveSearchResult[]>('/archive/search', data);
}

// PURPOSE: Get user's archive search history
export async function getArchiveSearchHistory(): Promise<ArchiveSearchHistoryEntry[]> {
  return apiClient.get<ArchiveSearchHistoryEntry[]>('/archive/search/history');
}
