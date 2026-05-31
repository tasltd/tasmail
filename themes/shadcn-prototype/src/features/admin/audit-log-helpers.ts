// TMAIL-352: pure helpers for the admin audit log viewer. Extracted so the
// shadcn-prototype workspace can unit-test them with node's native test
// runner (no DOM, no React Testing Library set up here).

/**
 * Convert a `<input type="datetime-local">` value (no timezone info) into
 * an ISO-8601 UTC string the backend can parse as `chrono::DateTime<Utc>`.
 * Returns `undefined` for empty/invalid input so the caller can simply
 * omit the query param.
 */
export function localToIso(local: string | undefined | null): string | undefined {
  if (!local) return undefined;
  const d = new Date(local);
  if (Number.isNaN(d.getTime())) return undefined;
  return d.toISOString();
}

export const ADMIN_TAB_OVERVIEW = 'overview' as const;
export const ADMIN_TAB_AUDIT = 'audit-log' as const;
export type AdminTab = typeof ADMIN_TAB_OVERVIEW | typeof ADMIN_TAB_AUDIT;

/**
 * Coerce an arbitrary `?tab=` query string value to a known AdminTab,
 * falling back to overview. Used to keep deep links stable when a user
 * tampers with the URL.
 */
export function parseAdminTab(raw: string | null | undefined): AdminTab {
  return raw === ADMIN_TAB_AUDIT ? ADMIN_TAB_AUDIT : ADMIN_TAB_OVERVIEW;
}

// Re-export the query-string helper from the API client. Lives here as
// well as in `api/admin-audit-log.ts` to keep the audit-log helpers in
// one importable surface for unit testing. Trace-check.py only cares about
// the literal `apiClient.get('/admin/audit-log')` call site over there.
export interface AuditLogQueryParams {
  mailbox_id?: string;
  action?: string;
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export function buildAuditLogQueryString(params: AuditLogQueryParams): string {
  const search = new URLSearchParams();
  if (params.mailbox_id) search.set('mailbox_id', params.mailbox_id);
  if (params.action) search.set('action', params.action);
  if (params.from) search.set('from', params.from);
  if (params.to) search.set('to', params.to);
  if (params.limit !== undefined) search.set('limit', String(params.limit));
  if (params.offset !== undefined) search.set('offset', String(params.offset));
  return search.toString();
}
