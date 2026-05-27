// Added: Bulk user import API functions for TMAIL-136
import { apiClient } from './client';

// Added: TypeScript interface for bulk import record
export interface BulkUserImport {
  id: string;
  admin_id: string;
  filename: string;
  total_rows: number;
  processed_rows: number;
  success_count: number;
  error_count: number;
  errors: BulkImportError[];
  status: 'pending' | 'processing' | 'completed' | 'failed';
  created_at: string;
  completed_at: string | null;
}

// Added: TypeScript interface for row-level import error
export interface BulkImportError {
  row: number;
  field: string;
  message: string;
}

// NOTE: Upload uses FormData (multipart) instead of JSON — requires custom fetch call
export const bulkImportApi = {
  // Added: Upload a CSV file for bulk user provisioning
  upload: async (file: File): Promise<BulkUserImport> => {
    const formData = new FormData();
    formData.append('file', file);
    // NOTE: Use raw fetch for multipart — apiClient.post sets Content-Type: application/json
    const token = apiClient.getToken();
    const response = await fetch('/api/admin/users/bulk-import', {
      method: 'POST',
      headers: token ? { Authorization: `Bearer ${token}` } : {},
      body: formData,
    });
    if (!response.ok) {
      const errorBody = await response.text();
      throw new Error(`Upload failed (${response.status}): ${errorBody}`);
    }
    return response.json();
  },

  // Added: List all bulk import records for the current admin
  list: (): Promise<BulkUserImport[]> =>
    apiClient.get<BulkUserImport[]>('/admin/users/bulk-imports'),

  // Added: Get a single bulk import record by ID with error details
  get: (id: string): Promise<BulkUserImport> =>
    apiClient.get<BulkUserImport>(`/admin/users/bulk-imports/${id}`),

  // Added: Download CSV template file
  downloadTemplate: async (): Promise<void> => {
    const token = apiClient.getToken();
    const response = await fetch('/api/admin/users/bulk-import/template', {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    });
    if (!response.ok) {
      throw new Error('Failed to download template');
    }
    // Added: Trigger browser download of the CSV template
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'bulk-import-template.csv';
    anchor.click();
    URL.revokeObjectURL(url);
  },

  // Added: Export all users as CSV via GET /api/admin/users/export (TMAIL-136)
  exportUsers: async (): Promise<void> => {
    const token = apiClient.getToken();
    const response = await fetch('/api/admin/users/export', {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    });
    if (!response.ok) {
      const errorBody = await response.text();
      throw new Error(`Export failed (${response.status}): ${errorBody}`);
    }
    const blob = await response.blob();
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = 'users-export.csv';
    anchor.click();
    URL.revokeObjectURL(url);
  },
};
