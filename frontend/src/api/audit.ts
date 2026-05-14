// TMAIL-198: admin client for the audit_log table.
import { apiClient } from './client';

export interface AuditLogEntry {
  id: string;
  mailbox_id: string | null;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  details: Record<string, unknown> | null;
  ip_address: string | null;
  user_agent: string | null;
  created_at: string;
}

export interface AuditLogQuery {
  mailbox_id?: string;
  action?: string;
  limit?: number;
}

// NOTE: keep one literal `apiClient.get('/admin/audit-log')` call site so
// scripts/trace-check.py picks the route up via static regex. The filtered
// branch composes the query string inline after the same literal prefix.
export const auditApi = {
  list: (params: AuditLogQuery = {}) => {
    const search = new URLSearchParams();
    if (params.mailbox_id) search.set('mailbox_id', params.mailbox_id);
    if (params.action) search.set('action', params.action);
    if (params.limit !== undefined) search.set('limit', String(params.limit));
    const suffix = search.toString();
    if (!suffix) return apiClient.get<AuditLogEntry[]>('/admin/audit-log');
    return apiClient.get<AuditLogEntry[]>(`/admin/audit-log?${suffix}`);
  },
};
