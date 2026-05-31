// TMAIL-354: Modern UI Admin → Retention sub-tab API client. CRUD against
// /api/admin/retention — matches the Rust handlers in
// backend/src/handlers/retention.rs and the model in
// backend/src/models/retention_policy.rs.
import { apiClient } from './client';

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

export const adminRetentionApi = {
  list: () => apiClient.get<RetentionPolicy[]>('/admin/retention'),
  create: (body: CreateRetentionPolicyRequest) =>
    apiClient.post<RetentionPolicy>('/admin/retention', body),
  update: (id: string, body: UpdateRetentionPolicyRequest) =>
    apiClient.put<RetentionPolicy>(`/admin/retention/${id}`, body),
  delete: (id: string) => apiClient.delete<void>(`/admin/retention/${id}`),
};
