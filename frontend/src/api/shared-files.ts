// Added: Shared files API module for large file sharing via download links (TMAIL-138)
import { apiClient } from './client';
import { API_BASE_URL } from '../utils/constants';

/// PURPOSE: Represents a shared file with download link metadata
export interface SharedFile {
  id: string;
  user_id: string;
  filename: string;
  content_type: string;
  file_size: number;
  storage_path: string;
  download_token: string;
  download_count: number;
  max_downloads: number | null;
  expires_at: string | null;
  password_hash: string | null;
  created_at: string;
}

/// PURPOSE: Upload a file for sharing via multipart/form-data
/// CONSTRAINTS: File field is required; max_downloads, expires_at, password are optional
/// EXTERNAL: Uses fetch directly for FormData (apiClient sets Content-Type: application/json)
export async function uploadSharedFile(formData: FormData): Promise<SharedFile> {
  const token = apiClient.getToken();
  const response = await fetch(`${API_BASE_URL}/shared-files/upload`, {
    method: 'POST',
    headers: token ? { Authorization: `Bearer ${token}` } : {},
    body: formData,
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Upload failed (${response.status}): ${body}`);
  }

  return response.json();
}

/// PURPOSE: List all shared files for the current user
export async function listSharedFiles(): Promise<SharedFile[]> {
  return apiClient.get<SharedFile[]>('/shared-files');
}

/// PURPOSE: Get details of a specific shared file
export async function getSharedFile(id: string): Promise<SharedFile> {
  return apiClient.get<SharedFile>(`/shared-files/${id}`);
}

/// PURPOSE: Delete a shared file (removes record and disk file)
export async function deleteSharedFile(id: string): Promise<void> {
  await apiClient.delete(`/shared-files/${id}`);
}

/// PURPOSE: Generate the public download URL for a shared file
/// NOTE: This URL can be shared with anyone — no auth required
export function getDownloadUrl(downloadToken: string): string {
  return `${API_BASE_URL}/dl/${downloadToken}`;
}
