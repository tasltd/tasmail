// Added: Attachment API module for TMAIL-59 — upload, download, list, delete, stats
import { apiClient } from './client';
import { API_BASE_URL } from '../utils/constants';

/**
 * PURPOSE: Represents a stored attachment with virus scan metadata
 * EXTERNAL: Maps to backend Attachment struct from models/attachment.rs
 */
export interface Attachment {
  id: string;
  mailbox_id: string;
  message_uid: number | null;
  folder: string | null;
  filename: string;
  content_type: string;
  size_bytes: number;
  storage_path: string;
  checksum: string;
  scan_status: 'pending' | 'clean' | 'infected' | 'error';
  scan_result: string | null;
  scanned_at: string | null;
  created_at: string;
}

/**
 * PURPOSE: Aggregated storage statistics for a mailbox
 */
export interface StorageStats {
  total_count: number;
  total_size_bytes: number;
  pending_scans: number;
  infected_count: number;
}

export const attachmentsApi = {
  /**
   * PURPOSE: Upload a file as an attachment via multipart/form-data
   * CONSTRAINTS: Max file size enforced server-side; bypasses JSON content-type
   * NOTE: Uses raw fetch instead of apiClient.post because multipart requires FormData
   */
  upload: async (file: File): Promise<Attachment> => {
    const formData = new FormData();
    formData.append('file', file);

    const token = apiClient.getToken();
    const headers: Record<string, string> = {};
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }
    // NOTE: Do not set Content-Type — browser sets it with boundary for multipart

    const response = await fetch(`${API_BASE_URL}/attachments`, {
      method: 'POST',
      headers,
      body: formData,
    });

    if (!response.ok) {
      const body = await response.text();
      throw new Error(`Upload failed (${response.status}): ${body}`);
    }

    return response.json();
  },

  /** PURPOSE: List all attachments for current user */
  list: () => apiClient.get<Attachment[]>('/attachments'),

  /**
   * PURPOSE: Download attachment file as Blob
   * NOTE: Uses raw fetch for binary response handling
   */
  download: async (attachmentId: string): Promise<Blob> => {
    const token = apiClient.getToken();
    const headers: Record<string, string> = {};
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

    const response = await fetch(
      `${API_BASE_URL}/attachments/${attachmentId}/download`,
      { headers },
    );

    if (!response.ok) {
      const body = await response.text();
      throw new Error(`Download failed (${response.status}): ${body}`);
    }

    return response.blob();
  },

  /** PURPOSE: Delete an attachment (file + DB record) */
  delete: (attachmentId: string) =>
    apiClient.delete<void>(`/attachments/${attachmentId}`),

  /** PURPOSE: Get storage usage statistics for current user */
  stats: () => apiClient.get<StorageStats>('/attachments/stats'),
};
