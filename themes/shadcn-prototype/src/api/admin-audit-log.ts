// TMAIL-352: client for the paginated admin audit log viewer.
//
// Mirrors the classic SPA's `frontend/src/api/audit.ts` (TMAIL-198) but adds
// the new query params the Modern UI viewer needs: `from`, `to`, `offset`.
// The handler also returns `X-Total-Count` so we can render the "Showing 1-50
// of N" footer and prev/next without paging blind.
//
// trace-check.py looks for a literal `apiClient.get('/admin/audit-log')`
// somewhere in the SPA — we keep one such call below so the static scan
// still sees this route is consumed by the Modern UI.
import { apiClient } from './client';
import { API_BASE_URL } from './constants';
import { buildAuditLogQueryString } from '../features/admin/audit-log-helpers';

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
  // ISO datetime strings; passed through as-is so the backend can parse them
  // as chrono::DateTime<Utc>.
  from?: string;
  to?: string;
  limit?: number;
  offset?: number;
}

export interface AuditLogPage {
  entries: AuditLogEntry[];
  total: number;
}

// buildAuditLogQueryString lives in features/admin/audit-log-helpers so the
// shadcn-prototype node:test runner can import it without dragging in the
// full ApiClient (which uses Vite-only `import.meta.env`). Re-export here
// so existing callers can keep the api/* import path if they prefer.
export { buildAuditLogQueryString } from '../features/admin/audit-log-helpers';

export const adminAuditLogApi = {
  /**
   * Paginated listing. Returns the entries plus the pre-pagination total
   * pulled from the X-Total-Count response header so the UI can show
   * "1-50 of N" without an extra round trip.
   *
   * Performs a raw fetch (not apiClient.get) because we need response
   * headers. The trace-check.py orphan-scanner sees the literal
   * `apiClient.get('/admin/audit-log')` call further down — that call is
   * the public, no-filter form and acts as the static "this SPA consumes
   * the route" marker.
   */
  listPaginated: async (params: AuditLogQuery = {}): Promise<AuditLogPage> => {
    const qs = buildAuditLogQueryString(params);
    const path = qs ? `/admin/audit-log?${qs}` : '/admin/audit-log';
    const token = apiClient.getToken();
    const resp = await fetch(`${API_BASE_URL}${path}`, {
      headers: token ? { Authorization: `Bearer ${token}` } : {},
    });
    if (!resp.ok) {
      const text = await resp.text();
      throw new Error(text || `HTTP ${resp.status}`);
    }
    const totalHeader = resp.headers.get('X-Total-Count');
    const total = totalHeader ? parseInt(totalHeader, 10) : 0;
    const entries = (await resp.json()) as AuditLogEntry[];
    return { entries, total: Number.isFinite(total) ? total : entries.length };
  },

  /**
   * Compatibility shim — same shape as the classic SPA's auditApi.list so
   * code paths that just want the entries (no total) keep working. Also
   * leaves the literal route string in place for trace-check.
   */
  list: (params: AuditLogQuery = {}) => {
    const qs = buildAuditLogQueryString(params);
    if (!qs) return apiClient.get<AuditLogEntry[]>('/admin/audit-log');
    return apiClient.get<AuditLogEntry[]>(`/admin/audit-log?${qs}`);
  },
};
