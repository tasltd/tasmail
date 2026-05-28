// Added: EML import/export API functions for TMAIL-68
import { API_BASE_URL } from '../utils/constants';

/**
 * PURPOSE: Export a single email as a raw .eml file (RFC822 format)
 * CONSTRAINTS: Requires valid folder name and message UID; uses raw fetch for binary response
 * EXTERNAL: GET /api/folders/{folder}/messages/{uid}/eml — returns message/rfc822 blob
 */
export async function exportEml(folder: string, uid: number): Promise<Blob> {
  const encodedFolder = encodeURIComponent(folder);
  const response = await fetch(
    `${API_BASE_URL}/folders/${encodedFolder}/messages/${uid}/eml`,
    {
      headers: {
        Authorization: `Bearer ${localStorage.getItem('access_token')}`,
      },
    },
  );

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(
      `EML export failed for UID ${uid} in folder '${folder}': ${response.status} — ${errorText}`,
    );
  }

  return response.blob();
}

/**
 * PURPOSE: Import a .eml file into a specified IMAP folder
 * CONSTRAINTS: File must be a valid .eml (RFC822) file; body sent as raw bytes
 * EXTERNAL: POST /api/folders/{folder}/import-eml — accepts message/rfc822 body
 */
export async function importEml(folder: string, file: File): Promise<void> {
  const encodedFolder = encodeURIComponent(folder);
  const arrayBuffer = await file.arrayBuffer();

  const response = await fetch(
    `${API_BASE_URL}/folders/${encodedFolder}/import-eml`,
    {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${localStorage.getItem('access_token')}`,
        'Content-Type': 'message/rfc822',
      },
      body: arrayBuffer,
    },
  );

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(
      `EML import failed for folder '${folder}': ${response.status} — ${errorText}`,
    );
  }
}

/**
 * PURPOSE: Trigger a browser file download from a Blob
 * CONSTRAINTS: Creates and clicks a temporary <a> element; revokes object URL after download
 */
export function downloadEml(blob: Blob, uid: number): void {
  const objectUrl = URL.createObjectURL(blob);
  const anchorElement = document.createElement('a');
  anchorElement.href = objectUrl;
  anchorElement.download = `message_${uid}.eml`;
  anchorElement.click();
  URL.revokeObjectURL(objectUrl);
}

/**
 * PURPOSE: Export an entire folder as an MBOX file (RFC 4155 format)
 * CONSTRAINTS: Requires valid folder name; streams the full folder so requests may be large
 * EXTERNAL: GET /api/folders/{folder}/export-mbox — returns application/mbox blob
 */
export async function exportFolderMbox(folder: string): Promise<Blob> {
  const encodedFolder = encodeURIComponent(folder);
  const response = await fetch(
    `${API_BASE_URL}/folders/${encodedFolder}/export-mbox`,
    {
      headers: {
        Authorization: `Bearer ${localStorage.getItem('access_token')}`,
      },
    },
  );

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(
      `MBOX export failed for folder '${folder}': ${response.status} — ${errorText}`,
    );
  }

  return response.blob();
}

/**
 * PURPOSE: Trigger a browser file download for an MBOX folder export
 * CONSTRAINTS: Sanitises the folder name into a safe `.mbox` filename
 */
export function downloadMbox(blob: Blob, folder: string): void {
  const safeName = folder.replace(/[\\/"\n\r\0]/g, '_').trim() || 'folder';
  const objectUrl = URL.createObjectURL(blob);
  const anchorElement = document.createElement('a');
  anchorElement.href = objectUrl;
  anchorElement.download = `${safeName}.mbox`;
  anchorElement.click();
  URL.revokeObjectURL(objectUrl);
}
