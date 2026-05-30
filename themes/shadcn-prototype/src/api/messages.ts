import type { FullMessage, MessageListResponse, SearchResponse, SendEmailRequest } from '../types/mail';
import { API_BASE_URL } from './constants';
import { ApiError, apiClient } from './client';

// Added: Advanced search filter parameters for TMAIL-32
export interface AdvancedSearchParams {
  query: string;
  folder?: string;
  from?: string;
  to?: string;
  subject?: string;
  dateFrom?: string;  // ISO date string (YYYY-MM-DD)
  dateTo?: string;    // ISO date string (YYYY-MM-DD)
  hasAttachment?: boolean;
  isUnread?: boolean;
  isStarred?: boolean;
}

export async function fetchMessages(
  folder: string,
  page = 0,
  pageSize = 50,
): Promise<MessageListResponse> {
  return apiClient.get<MessageListResponse>(
    `/folders/${encodeURIComponent(folder)}/messages?page=${page}&page_size=${pageSize}`,
  );
}

export async function fetchMessage(
  folder: string,
  uid: number,
): Promise<FullMessage> {
  return apiClient.get<FullMessage>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}`,
  );
}

export async function sendMessage(request: SendEmailRequest): Promise<void> {
  await apiClient.post('/messages/send', request);
}

// Changed: Accept AdvancedSearchParams or simple query+folder for backward compatibility
export async function searchMessages(
  queryOrParams: string | AdvancedSearchParams,
  folder?: string,
): Promise<SearchResponse> {
  const params = new URLSearchParams();

  if (typeof queryOrParams === 'string') {
    // NOTE: Legacy call signature — simple query string
    params.set('q', queryOrParams);
    if (folder) params.set('folder', folder);
  } else {
    // Added: Serialize all non-empty advanced search params
    const advancedSearchParams = queryOrParams;
    if (advancedSearchParams.query) params.set('q', advancedSearchParams.query);
    if (advancedSearchParams.folder) params.set('folder', advancedSearchParams.folder);
    if (advancedSearchParams.from) params.set('from', advancedSearchParams.from);
    if (advancedSearchParams.to) params.set('to', advancedSearchParams.to);
    if (advancedSearchParams.subject) params.set('subject', advancedSearchParams.subject);
    if (advancedSearchParams.dateFrom) params.set('date_from', advancedSearchParams.dateFrom);
    if (advancedSearchParams.dateTo) params.set('date_to', advancedSearchParams.dateTo);
    if (advancedSearchParams.hasAttachment) params.set('has_attachment', 'true');
    if (advancedSearchParams.isUnread) params.set('is_unread', 'true');
    if (advancedSearchParams.isStarred) params.set('is_starred', 'true');
  }

  return apiClient.get<SearchResponse>(`/search?${params.toString()}`);
}

export async function deleteMessage(folder: string, uid: number): Promise<void> {
  await apiClient.delete(`/folders/${encodeURIComponent(folder)}/messages/${uid}`);
}

export async function moveMessage(
  folder: string,
  uid: number,
  toFolder: string,
): Promise<void> {
  await apiClient.post(`/folders/${encodeURIComponent(folder)}/messages/${uid}/move`, {
    to_folder: toFolder,
  });
}

export async function flagMessage(
  folder: string,
  uid: number,
  flag: string,
  add: boolean,
): Promise<void> {
  await apiClient.post(`/folders/${encodeURIComponent(folder)}/messages/${uid}/flag`, {
    flag,
    add,
  });
}

export interface SaveDraftRequest {
  to: string[];
  cc?: string[];
  subject: string;
  html_body?: string;
  text_body?: string;
  // TMAIL-319: optional RFC 5322 §3.6.4 threading headers so a draft started
  // from Reply / Reply All / Forward remembers which conversation it belongs
  // to. The backend /api/drafts handler is welcome to ignore them today —
  // the field exists so the wire shape is forward-compatible without a
  // separate ComposeModal branch.
  in_reply_to?: string;
  references?: string[];
}

export async function saveDraft(request: SaveDraftRequest): Promise<void> {
  await apiClient.post('/drafts', request);
}

// Added (TMAIL-320): fetch an attachment's raw bytes as a Blob so the
// EmailReader Download button can save it to disk via a temporary object URL.
// The shared ApiClient is JSON-only — for binary payloads we hand-roll the
// fetch and re-use the bearer token the client already holds. Returns the
// Blob plus the server-provided filename so the SPA doesn't have to re-derive
// it from Content-Disposition.
export async function downloadAttachment(
  folder: string,
  uid: number,
  partId: string,
  fallbackFilename: string,
): Promise<{ blob: Blob; filename: string }> {
  const headers: Record<string, string> = {};
  const token = apiClient.getToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const url = `${API_BASE_URL}/folders/${encodeURIComponent(
    folder,
  )}/messages/${uid}/parts/${encodeURIComponent(partId)}`;
  const resp = await fetch(url, { headers });

  if (!resp.ok) {
    const body = await resp.text().catch(() => '');
    throw new ApiError(resp.status, body || `attachment fetch failed`);
  }

  // RFC 6266: prefer filename* (UTF-8) over the ASCII filename fallback. Both
  // are present because the backend emits both — see download_message_part.
  const cd = resp.headers.get('content-disposition') ?? '';
  const filename = parseContentDispositionFilename(cd) ?? fallbackFilename;

  const blob = await resp.blob();
  return { blob, filename };
}

// Exported so unit tests can exercise the RFC 6266 parsing without a network
// round-trip — keeping it a top-level helper also matches the codebase's
// "one concept per file is fine, but small pure helpers stay alongside their
// only caller" convention (cf. parseReplyContext in replyContext.ts).
export function parseContentDispositionFilename(
  headerValue: string,
): string | null {
  if (!headerValue) return null;
  // Prefer filename*=UTF-8'...'<percent-encoded> per RFC 5987.
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
