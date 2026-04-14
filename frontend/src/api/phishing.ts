// Added: Phishing scan API functions and types for TMAIL-124

import { apiClient } from './client';

// Added: Suspicious link detail returned by the phishing scanner
export interface SuspiciousLink {
  url: string;
  display_text: string;
  reasons: string[];
}

// Added: Full phishing report for a scanned message
export interface PhishingReport {
  id: string;
  mailbox_id: string;
  message_uid: number;
  folder: string;
  suspicious_links: SuspiciousLink[];
  suspicious_sender: boolean;
  spoofed_display_name: boolean;
  risk_score: number;
  user_action: string;
  created_at: string;
}

// Added: Request body for triggering a phishing scan
export interface ScanRequest {
  html_body: string;
  sender_display_name: string;
  sender_email: string;
}

// Added: Request body for updating user action on a phishing report
export interface UpdateActionRequest {
  action: 'dismissed' | 'reported' | 'confirmed_safe';
}

/**
 * PURPOSE: Fetch existing phishing report for a specific message
 * CONSTRAINTS: Returns null if message has not been scanned yet
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
 * PURPOSE: Trigger a phishing scan on a message and persist the result
 * CONSTRAINTS: Requires html_body, sender_display_name, and sender_email
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
 * PURPOSE: Update user action on a phishing report (dismiss, report, or confirm safe)
 */
export async function updatePhishingAction(
  reportId: string,
  request: UpdateActionRequest,
): Promise<void> {
  await apiClient.put(`/phishing/${reportId}/action`, request);
}
