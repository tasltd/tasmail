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
