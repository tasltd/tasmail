// TMAIL-231: ported from frontend/src/api/admin-users.ts so AdminDashboard
// in the alt-UI can hit the same /api/admin/users endpoints.
import { apiClient } from './client';
import { API_BASE_URL } from './constants';

export interface UserInfo {
  id: string;
  domain_id: string;
  username: string;
  display_name: string | null;
  quota_bytes: number;
  quota_warn_percent: number;
  active: boolean;
  is_admin: boolean;
  created_at: string;
}

export interface CreateUserRequest {
  username: string;
  password: string;
  domain_id: string;
  display_name?: string;
  quota_bytes?: number;
}

export interface BulkImportRow {
  email: string;
  display_name: string;
  status: string;
  error_message: string | null;
}

export interface BulkImportResult {
  id: string;
  filename: string;
  total_rows: number;
  success_count: number;
  error_count: number;
  status: string;
  rows?: BulkImportRow[];
}

export const adminUsersApi = {
  list: () => apiClient.get<UserInfo[]>('/admin/users'),
  create: (body: CreateUserRequest) => apiClient.post<UserInfo>('/admin/users', body),
  delete: (id: string) => apiClient.delete<void>(`/admin/users/${id}`),
  bulkImport: async (file: File): Promise<BulkImportResult> => {
    const fd = new FormData();
    fd.append('file', file, file.name);
    const token = localStorage.getItem('access_token') ?? '';
    const resp = await fetch(`${API_BASE_URL}/admin/users/bulk-import`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token}` },
      body: fd,
    });
    if (!resp.ok) {
      const text = await resp.text();
      throw new Error(text || `HTTP ${resp.status}`);
    }
    return resp.json();
  },
};
