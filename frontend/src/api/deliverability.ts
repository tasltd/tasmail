// Added: Deliverability API client for email deliverability testing (TMAIL-39)

import { apiClient } from './client';

// Added: Status of an individual deliverability check
export type CheckStatus = 'pass' | 'fail' | 'warn' | 'error';

// Added: A single deliverability check result
export interface CheckResult {
  name: string;
  status: CheckStatus;
  details: string;
}

// Added: Full deliverability report with score and check results
export interface DeliverabilityReport {
  domain: string;
  checks: CheckResult[];
  score: number;
}

// PURPOSE: Run deliverability checks for a domain (admin only)
export async function runDeliverabilityCheck(domain: string): Promise<DeliverabilityReport> {
  return apiClient.get<DeliverabilityReport>(
    `/admin/deliverability/check?domain=${encodeURIComponent(domain)}`,
  );
}

// === TMAIL-39 — external deliverability tools (mail-tester + Postmaster Tools) ===

// Added: TMAIL-39 — single-use mail-tester.com handle returned by the backend.
// The user sends mail to `test_address`, then opens `report_url` within
// `expires_in_minutes` to see the 0–10 spam score.
export interface MailTesterHandle {
  test_address: string;
  report_url: string;
  expires_in_minutes: number;
  instructions: string;
}

// Added: TMAIL-39 — Postmaster Tools deep-link + setup hint.
export interface PostmasterTools {
  dashboard_url: string;
  instructions: string;
}

// Added: TMAIL-39 — per-provider manual checklist entry (Gmail/Outlook/Yahoo/ProtonMail).
export interface ProviderCheck {
  name: string;
  spam_folder_label: string;
  instructions: string;
}

// Added: TMAIL-39 — composite payload powering the External Tools panel.
export interface ExternalToolsResponse {
  mail_tester: MailTesterHandle;
  google_postmaster: PostmasterTools;
  providers: ProviderCheck[];
}

// PURPOSE: TMAIL-39 — fetch the external deliverability tools panel data. Each call
// mints a FRESH mail-tester handle, so the UI should treat this as a "generate" action,
// not as a long-lived query that re-uses cached data across re-runs.
export async function getExternalDeliverabilityTools(
  domain: string,
): Promise<ExternalToolsResponse> {
  const query = domain.trim()
    ? `?domain=${encodeURIComponent(domain.trim())}`
    : '';
  return apiClient.get<ExternalToolsResponse>(
    `/admin/deliverability/external-tools${query}`,
  );
}
