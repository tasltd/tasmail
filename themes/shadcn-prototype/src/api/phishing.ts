// TMAIL-347: Phishing API client for the Modern UI EmailReader.
//
// Mirrors `frontend/src/api/phishing.ts` (the classic SPA's client) so the
// two surfaces hit the exact same backend contract — see
// `backend/src/handlers/phishing.rs` for the route handlers wired in
// `router.rs:397-407`.
//
// Endpoints consumed:
//   GET  /api/folders/{folder}/messages/{uid}/phishing
//   POST /api/folders/{folder}/messages/{uid}/phishing/scan
//   PUT  /api/phishing/{id}/action
import { apiClient } from './client';

export interface SuspiciousLink {
  url: string;
  display_text: string;
  reasons: string[];
}

export interface DangerousAttachment {
  filename: string;
  extension: string;
  reason: string;
}

export interface AttachmentMeta {
  filename: string;
  content_type?: string;
}

export interface PhishingReport {
  id: string;
  mailbox_id: string;
  message_uid: number;
  folder: string;
  suspicious_links: SuspiciousLink[];
  suspicious_sender: boolean;
  spoofed_display_name: boolean;
  risk_score: number;
  user_action: 'none' | 'dismissed' | 'reported' | 'confirmed_safe' | string;
  dangerous_attachments?: DangerousAttachment[];
  created_at: string;
}

export interface ScanRequest {
  html_body: string;
  sender_display_name: string;
  sender_email: string;
  attachments?: AttachmentMeta[];
}

export type PhishingAction = 'dismissed' | 'reported' | 'confirmed_safe';

export interface UpdateActionRequest {
  action: PhishingAction;
}

/**
 * Fetch the existing phishing report for a message. Returns `null` when the
 * message has not been scanned yet — TanStack will still cache the `null` so
 * we don't re-fetch on every render.
 */
export async function getPhishingReport(
  folder: string,
  uid: number,
): Promise<PhishingReport | null> {
  return apiClient.get<PhishingReport | null>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}/phishing`,
  );
}

/**
 * Trigger a scan and persist the result. Returns the created report so the
 * UI can render the banner immediately without waiting for a re-fetch.
 */
export async function scanMessage(
  folder: string,
  uid: number,
  request: ScanRequest,
): Promise<PhishingReport> {
  return apiClient.post<PhishingReport>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}/phishing/scan`,
    request,
  );
}

/**
 * Update the user action on an existing phishing report — "Mark safe"
 * (confirmed_safe), "Report" (reported), or dismiss the banner.
 */
export async function updatePhishingAction(
  reportId: string,
  action: PhishingAction,
): Promise<void> {
  await apiClient.put<void>(`/phishing/${reportId}/action`, { action });
}

/**
 * Helper — extract `Name` and `email@host` from a `Name <email@host>`
 * envelope-style From header. Used by the scan request builder so the modern
 * UI can pass the same `sender_display_name` / `sender_email` shape the
 * classic SPA does. Pure function — exported for unit-test reach.
 */
export function parseFromHeader(from: string | null | undefined): {
  display: string;
  email: string;
} {
  if (!from) return { display: '', email: '' };
  const match = from.match(/^\s*(.*?)\s*<([^>]+)>\s*$/);
  if (match) {
    return { display: match[1].trim(), email: match[2].trim() };
  }
  // Bare address with no display name (e.g. "alice@example.com").
  return { display: from.trim(), email: from.trim() };
}
