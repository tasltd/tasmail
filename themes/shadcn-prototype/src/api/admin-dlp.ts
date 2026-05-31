// TMAIL-354: Modern UI Admin → DLP sub-tab API client. CRUD against
// /api/admin/dlp/* — matches the Rust handlers in
// backend/src/handlers/dlp.rs and the model in
// backend/src/models/dlp_rule.rs.
import { apiClient } from './client';

export type DlpAction = 'block' | 'quarantine' | 'warn' | 'log';
export type DlpSeverity = 'low' | 'medium' | 'high' | 'critical';

export interface DlpRule {
  id: string;
  name: string;
  description: string | null;
  pattern: string;
  pattern_type: string;
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
  description?: string;
  pattern: string;
  pattern_type?: string;
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
  pattern_type?: string;
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

export interface ListViolationsParams {
  limit?: number;
  offset?: number;
}

function buildViolationsQs(params: ListViolationsParams = {}): string {
  const qs = new URLSearchParams();
  if (params.limit !== undefined) qs.set('limit', String(params.limit));
  if (params.offset !== undefined) qs.set('offset', String(params.offset));
  return qs.toString();
}

export const adminDlpApi = {
  listRules: () => apiClient.get<DlpRule[]>('/admin/dlp/rules'),
  createRule: (body: CreateDlpRuleRequest) =>
    apiClient.post<DlpRule>('/admin/dlp/rules', body),
  updateRule: (id: string, body: UpdateDlpRuleRequest) =>
    apiClient.put<DlpRule>(`/admin/dlp/rules/${id}`, body),
  deleteRule: (id: string) => apiClient.delete<void>(`/admin/dlp/rules/${id}`),
  listViolations: (params: ListViolationsParams = {}) => {
    const qs = buildViolationsQs(params);
    return apiClient.get<DlpViolation[]>(
      qs ? `/admin/dlp/violations?${qs}` : '/admin/dlp/violations',
    );
  },
  // POST /api/admin/dlp/scan — dry-run a body/subject against active rules.
  testScan: (body: DlpScanRequest) =>
    apiClient.post<DlpScanMatch[]>('/admin/dlp/scan', body),
};
