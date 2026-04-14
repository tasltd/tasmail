// Added: PST import API functions for TMAIL-115
import type { PstImport } from '../types/pst-import';
import { apiClient } from './client';

// NOTE: Upload uses FormData (multipart) instead of JSON — requires custom fetch call
export const pstImportApi = {
  // Added: Upload a PST file with optional target folder
  upload: async (file: File, targetFolder?: string): Promise<PstImport> => {
    const formData = new FormData();
    formData.append('file', file);
    if (targetFolder) {
      formData.append('target_folder', targetFolder);
    }
    // NOTE: Use raw fetch for multipart — apiClient.post sets Content-Type: application/json
    const token = apiClient.getToken();
    const response = await fetch('/api/migration/pst/upload', {
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

  // Added: List all PST imports for the current user
  list: (): Promise<PstImport[]> => apiClient.get<PstImport[]>('/migration/pst'),

  // Added: Get a single PST import by ID
  get: (id: string): Promise<PstImport> => apiClient.get<PstImport>(`/migration/pst/${id}`),

  // Added: Delete/cancel a PST import
  delete: (id: string): Promise<void> => apiClient.delete(`/migration/pst/${id}`),
};
