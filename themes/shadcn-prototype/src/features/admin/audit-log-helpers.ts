// TMAIL-352: pure helpers for the admin audit log viewer. Extracted so the
// shadcn-prototype workspace can unit-test them with node's native test
// runner (no DOM, no React Testing Library set up here).
//
// TMAIL-353: extended with the Branding / SAML / OIDC / LDAP tab IDs so
// the same `parseAdminTab` keeps URL ↔ tab routing consistent across the
// whole AdminDashboard.

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
// Added (TMAIL-353): new sub-tabs for Branding + SSO + Directory.
export const ADMIN_TAB_BRANDING = 'branding' as const;
export const ADMIN_TAB_SAML = 'saml' as const;
export const ADMIN_TAB_OIDC = 'oidc' as const;
export const ADMIN_TAB_LDAP = 'ldap' as const;

export type AdminTab =
  | typeof ADMIN_TAB_OVERVIEW
  | typeof ADMIN_TAB_AUDIT
  | typeof ADMIN_TAB_BRANDING
  | typeof ADMIN_TAB_SAML
  | typeof ADMIN_TAB_OIDC
  | typeof ADMIN_TAB_LDAP;

// Single source of truth for the parser + the TabsList rendering order so
// adding a new tab is one edit, not three.
export const ADMIN_TABS: readonly AdminTab[] = [
  ADMIN_TAB_OVERVIEW,
  ADMIN_TAB_AUDIT,
  ADMIN_TAB_BRANDING,
  ADMIN_TAB_SAML,
  ADMIN_TAB_OIDC,
  ADMIN_TAB_LDAP,
] as const;

/**
 * Coerce an arbitrary `?tab=` query string value to a known AdminTab,
 * falling back to overview. Used to keep deep links stable when a user
 * tampers with the URL.
 */
export function parseAdminTab(raw: string | null | undefined): AdminTab {
  if (!raw) return ADMIN_TAB_OVERVIEW;
  return (ADMIN_TABS as readonly string[]).includes(raw) ? (raw as AdminTab) : ADMIN_TAB_OVERVIEW;
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
