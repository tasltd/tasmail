// Added: eDiscovery search API client for compliance investigations (TMAIL-137)

import { apiClient } from './client';

// Added: eDiscovery search status matching backend enum
export type EdiscoveryStatus = 'Pending' | 'Running' | 'Completed' | 'Failed' | 'Exported';

// Added: eDiscovery search interface matching backend model
export interface EdiscoverySearch {
  id: string;
  admin_id: string;
  name: string;
  description: string | null;
  search_query: string;
  target_users: string[] | null;
  date_from: string | null;
  date_to: string | null;
  include_attachments: boolean;
  status: EdiscoveryStatus;
  results_count: number | null;
  export_path: string | null;
  created_at: string;
  completed_at: string | null;
}

// Added: eDiscovery result interface matching backend model
export interface EdiscoveryResult {
  id: string;
  search_id: string;
  user_id: string;
  folder: string;
  uid: number;
  subject: string | null;
  from_address: string | null;
  date: string | null;
  snippet: string | null;
  relevance_score: number | null;
}

// Added: Combined search with results for detail endpoint
export interface EdiscoverySearchWithResults extends EdiscoverySearch {
  results: EdiscoveryResult[];
}

export interface CreateEdiscoveryRequest {
  name: string;
  description?: string;
  search_query: string;
  target_users?: string[];
  date_from?: string;
  date_to?: string;
  include_attachments?: boolean;
}

// PURPOSE: List all eDiscovery searches (admin only)
export async function listEdiscoverySearches(): Promise<EdiscoverySearch[]> {
  return apiClient.get<EdiscoverySearch[]>('/admin/ediscovery');
}

// PURPOSE: Create a new eDiscovery search (admin only)
export async function createEdiscoverySearch(data: CreateEdiscoveryRequest): Promise<EdiscoverySearch> {
  return apiClient.post<EdiscoverySearch>('/admin/ediscovery', data);
}

// PURPOSE: Get a single eDiscovery search with results (admin only)
export async function getEdiscoverySearch(id: string): Promise<EdiscoverySearchWithResults> {
  return apiClient.get<EdiscoverySearchWithResults>(`/admin/ediscovery/${id}`);
}

// PURPOSE: Delete an eDiscovery search (admin only)
export async function deleteEdiscoverySearch(id: string): Promise<void> {
  await apiClient.delete(`/admin/ediscovery/${id}`);
}

// PURPOSE: Execute an eDiscovery search across user mailboxes (admin only)
export async function executeEdiscoverySearch(id: string): Promise<EdiscoverySearch> {
  return apiClient.post<EdiscoverySearch>(`/admin/ediscovery/${id}/execute`, {});
}

// PURPOSE: Export eDiscovery search results to MBOX (admin only)
export async function exportEdiscoveryResults(id: string): Promise<EdiscoverySearch> {
  return apiClient.post<EdiscoverySearch>(`/admin/ediscovery/${id}/export`, {});
}
