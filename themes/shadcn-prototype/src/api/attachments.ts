// Added (TMAIL-321): attachments API client for the modern UI. Mirrors the
// classic frontend's `frontend/src/api/attachments.ts` so the two surfaces
// stay symmetric — upload via multipart, get back the persisted Attachment
// record (with `id`), then thread the `id` into POST /api/messages/schedule
// via the ScheduleSendRequest.attachment_ids field.
import { apiClient } from './client';
import { API_BASE_URL } from './constants';

/**
 * Represents a stored attachment with virus scan metadata.
 * Maps to the backend Attachment struct in models/attachment.rs.
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

export const attachmentsApi = {
  /**
   * Upload a single file via multipart/form-data. The browser fills the
   * boundary on the Content-Type header for us — explicitly setting it would
   * break the upload.
   */
  upload: async (file: File): Promise<Attachment> => {
    const formData = new FormData();
    formData.append('file', file);

    const token = apiClient.getToken();
    const headers: Record<string, string> = {};
    if (token) {
      headers['Authorization'] = `Bearer ${token}`;
    }

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

  /** Delete an attachment (file + DB record). Used when the user removes
   * an attachment from the composer BEFORE sending — orphaned uploads
   * (composer closed without sending) are cleaned up by the attachment
   * stats sweep on the next quota check. */
  delete: (attachmentId: string): Promise<void> =>
    apiClient.delete<void>(`/attachments/${attachmentId}`),
};
