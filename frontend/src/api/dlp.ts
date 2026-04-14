// Added: DLP API client for Data Loss Prevention rule management (TMAIL-108)

import { apiClient } from './client';

// Added: DLP action types matching backend dlp_action enum
export type DlpAction = 'block' | 'quarantine' | 'warn' | 'log';

// Added: DLP severity levels matching backend dlp_severity enum
export type DlpSeverity = 'low' | 'medium' | 'high' | 'critical';

// Added: DLP pattern types for rule matching strategy
export type DlpPatternType = 'regex' | 'keyword' | 'dictionary';

export interface DlpRule {
  id: string;
  name: string;
  description: string | null;
  pattern: string;
  pattern_type: DlpPatternType;
  action: DlpAction;
  severity: DlpSeverity;
  apply_to_subject: boolean;
  apply_to_body: boolean;
  apply_to_attachments: boolean;
  active: boolean;
  created_at: string;
  updated_at: string;
}

export interface DlpViolation {
  id: string;
  rule_id: string;
  user_id: string;
  action_taken: DlpAction;
  matched_pattern: string;
  matched_text: string | null;
  message_subject: string | null;
  recipient: string | null;
  created_at: string;
}

export interface CreateDlpRuleRequest {
  name: string;
  pattern: string;
  description?: string;
  pattern_type?: DlpPatternType;
  action?: DlpAction;
  severity?: DlpSeverity;
  apply_to_subject?: boolean;
  apply_to_body?: boolean;
  apply_to_attachments?: boolean;
}

export interface UpdateDlpRuleRequest {
  name?: string;
  description?: string;
  pattern?: string;
  pattern_type?: DlpPatternType;
  action?: DlpAction;
  severity?: DlpSeverity;
  apply_to_subject?: boolean;
  apply_to_body?: boolean;
  apply_to_attachments?: boolean;
  active?: boolean;
}

export interface DlpScanRequest {
  subject?: string;
  body?: string;
  recipient?: string;
}

export interface DlpScanMatch {
  rule_id: string;
  rule_name: string;
  action: DlpAction;
  severity: DlpSeverity;
  matched_pattern: string;
  matched_text: string;
}

// PURPOSE: List all DLP rules (admin endpoint)
export async function listDlpRules(): Promise<DlpRule[]> {
  return apiClient.get<DlpRule[]>('/admin/dlp/rules');
}

// PURPOSE: Create a new DLP rule
export async function createDlpRule(data: CreateDlpRuleRequest): Promise<DlpRule> {
  return apiClient.post<DlpRule>('/admin/dlp/rules', data);
}

// PURPOSE: Update an existing DLP rule by ID
export async function updateDlpRule(id: string, data: UpdateDlpRuleRequest): Promise<DlpRule> {
  return apiClient.put<DlpRule>(`/admin/dlp/rules/${id}`, data);
}

// PURPOSE: Delete a DLP rule and its violation records
export async function deleteDlpRule(id: string): Promise<void> {
  await apiClient.delete(`/admin/dlp/rules/${id}`);
}

// PURPOSE: List DLP violations with pagination
export async function listDlpViolations(limit = 50, offset = 0): Promise<DlpViolation[]> {
  return apiClient.get<DlpViolation[]>(`/admin/dlp/violations?limit=${limit}&offset=${offset}`);
}

// PURPOSE: Test scan text against all active DLP rules (dry-run, no violations recorded)
export async function testDlpScan(data: DlpScanRequest): Promise<DlpScanMatch[]> {
  return apiClient.post<DlpScanMatch[]>('/admin/dlp/scan', data);
}
