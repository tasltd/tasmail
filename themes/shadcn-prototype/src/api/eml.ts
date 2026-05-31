// TMAIL-349: per-message EML export + per-folder MBOX export / EML import for
// the alt-UI ("modern") theme. The shared ApiClient in `client.ts` is JSON-only,
// so the binary endpoints (GET .eml, GET .mbox, POST raw RFC822 bytes) hand-roll
// fetch and reuse the bearer token the client already holds. Mirrors the binary
// pattern the attachment-download path established in TMAIL-320
// (see `messages.ts::downloadAttachment`).
import { API_BASE_URL } from './constants';
import { ApiError, apiClient } from './client';

function authHeaders(extra: Record<string, string> = {}): Record<string, string> {
  const headers: Record<string, string> = { ...extra };
  const token = apiClient.getToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
}

// RFC 6266 filename extraction — same shape as `parseContentDispositionFilename`
// in messages.ts. Inlined (not imported) so this module stays self-contained and
// can be unit-tested without dragging in the rest of the messages-API surface.
export function parseContentDispositionFilename(
  headerValue: string,
): string | null {
  if (!headerValue) return null;
  const starMatch = headerValue.match(/filename\*\s*=\s*([^']*)'[^']*'([^;]+)/i);
  if (starMatch) {
    try {
      return decodeURIComponent(starMatch[2].trim());
    } catch {
      // fall through to the plain filename= form
    }
  }
  const plain = headerValue.match(/filename\s*=\s*"?([^";]+)"?/i);
  if (plain) return plain[1].trim();
  return null;
}

/**
 * Download a single message as a raw .eml file via
 * `GET /api/folders/{folder}/messages/{uid}/eml`.
 *
 * The backend (`handlers/eml.rs::export_eml`) returns the message body with
 * `Content-Type: message/rfc822` and `Content-Disposition: attachment;
 * filename="message_<uid>.eml"`. We return both the Blob and the server-
 * suggested filename so the caller can trigger a real browser download via a
 * temporary object URL — same pattern as `downloadAttachment` (TMAIL-320).
 */
export async function exportEml(
  folder: string,
  uid: number,
  fallbackFilename = `message_${uid}.eml`,
): Promise<{ blob: Blob; filename: string }> {
  const url = `${API_BASE_URL}/folders/${encodeURIComponent(folder)}/messages/${uid}/eml`;
  const resp = await fetch(url, { headers: authHeaders() });
  if (!resp.ok) {
    const body = await resp.text().catch(() => '');
    throw new ApiError(resp.status, body || 'EML export failed');
  }
  const cd = resp.headers.get('content-disposition') ?? '';
  const filename = parseContentDispositionFilename(cd) ?? fallbackFilename;
  const blob = await resp.blob();
  return { blob, filename };
}

/**
 * Download an entire folder as an mbox file via
 * `GET /api/folders/{folder}/export-mbox`.
 *
 * The backend (`handlers/eml.rs::export_folder_mbox`) concatenates every UID's
 * RFC822 body, prefixing each with an mboxo `From <sender> <ctime>` line and
 * escaping body lines that start with `From ` (RFC 4155 §2.2). The download
 * is named `<folder>.mbox`.
 */
export async function exportFolderMbox(
  folder: string,
  fallbackFilename?: string,
): Promise<{ blob: Blob; filename: string }> {
  const url = `${API_BASE_URL}/folders/${encodeURIComponent(folder)}/export-mbox`;
  const resp = await fetch(url, { headers: authHeaders() });
  if (!resp.ok) {
    const body = await resp.text().catch(() => '');
    throw new ApiError(resp.status, body || 'MBOX export failed');
  }
  const cd = resp.headers.get('content-disposition') ?? '';
  const filename =
    parseContentDispositionFilename(cd) ?? fallbackFilename ?? `${folder}.mbox`;
  const blob = await resp.blob();
  return { blob, filename };
}

/**
 * Upload a single .eml file into the target folder via
 * `POST /api/folders/{folder}/import-eml`.
 *
 * The body is the raw RFC822 bytes — Content-Type is forced to `message/rfc822`
 * so the backend's empty-body guard reads the request as binary rather than JSON.
 * The backend appends the message with `\Seen` set via IMAP APPEND.
 */
export interface ImportEmlResponse {
  message: string;
  folder: string;
  size: number;
}

export async function importEmlToFolder(
  folder: string,
  file: File | Blob,
): Promise<ImportEmlResponse> {
  const url = `${API_BASE_URL}/folders/${encodeURIComponent(folder)}/import-eml`;
  const resp = await fetch(url, {
    method: 'POST',
    headers: authHeaders({ 'Content-Type': 'message/rfc822' }),
    body: file,
  });
  if (!resp.ok) {
    const body = await resp.text().catch(() => '');
    throw new ApiError(resp.status, body || 'EML import failed');
  }
  const text = await resp.text();
  if (!text) {
    return { message: 'Imported', folder, size: file.size };
  }
  return JSON.parse(text) as ImportEmlResponse;
}

/**
 * Convenience helper: trigger a real browser download for an already-fetched
 * Blob via a temporary object URL. Same lifecycle the attachment-download path
 * uses — attach an anchor to the DOM, click it, revoke the URL.
 *
 * Kept module-local rather than exporting from a util barrel so the EML/MBOX
 * call sites stay one-liners.
 */
export function triggerBlobDownload(blob: Blob, filename: string): void {
  const objectUrl = URL.createObjectURL(blob);
  try {
    const a = document.createElement('a');
    a.href = objectUrl;
    a.download = filename;
    a.rel = 'noopener';
    document.body.appendChild(a);
    a.click();
    a.remove();
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}
