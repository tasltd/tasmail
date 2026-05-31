// TMAIL-354: Modern UI Admin → eDiscovery sub-tab API client. CRUD +
// execute + export against /api/admin/ediscovery — matches the Rust
// handlers in backend/src/handlers/ediscovery.rs and the model in
// backend/src/models/ediscovery.rs.
//
// All endpoints require the caller to be is_admin OR is_compliance_officer
// (see services/auth_service::require_compliance).
import { apiClient } from './client';

export type EdiscoveryStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'exported';

export type ExportFormat = 'mbox' | 'eml' | 'pdf';

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
  legal_hold_only: boolean;
  export_format: string;
  created_at: string;
  completed_at: string | null;
}

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
  legal_hold_only?: boolean;
  export_format?: ExportFormat;
}

export const adminEdiscoveryApi = {
  list: () => apiClient.get<EdiscoverySearch[]>('/admin/ediscovery'),
  get: (id: string) =>
    apiClient.get<EdiscoverySearchWithResults>(`/admin/ediscovery/${id}`),
  create: (body: CreateEdiscoveryRequest) =>
    apiClient.post<EdiscoverySearch>('/admin/ediscovery', body),
  delete: (id: string) => apiClient.delete<void>(`/admin/ediscovery/${id}`),
  execute: (id: string) =>
    apiClient.post<EdiscoverySearch>(`/admin/ediscovery/${id}/execute`, {}),
  export: (id: string, format?: ExportFormat) => {
    const path = format
      ? `/admin/ediscovery/${id}/export?format=${format}`
      : `/admin/ediscovery/${id}/export`;
    return apiClient.post<EdiscoverySearch>(path, {});
  },
};
