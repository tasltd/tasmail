// TMAIL-198: admin viewer for the audit_log table.
//
// Action-prefix dropdown (auth.* / billing.* / admin.* / all) plus a free-form
// action filter and a configurable limit. Renders a single table with the
// most useful columns first; details + user-agent collapse into a popover-ish
// inline expansion to keep rows readable.
import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ScrollText, RefreshCw } from 'lucide-react';
import { auditApi, type AuditLogEntry } from '../../api/audit';

const PREFIXES: { label: string; value: string }[] = [
  { label: 'All', value: '' },
  { label: 'auth.*', value: 'auth.' },
  { label: 'billing.*', value: 'billing.' },
  { label: 'admin.*', value: 'admin.' },
];

function formatDate(iso: string): string {
  return new Date(iso).toLocaleString();
}

export function AuditLogManager() {
  const [prefix, setPrefix] = useState('');
  const [actionFilter, setActionFilter] = useState('');
  const [limit, setLimit] = useState(100);

  const effectiveAction = actionFilter || prefix || undefined;

  const query = useQuery<AuditLogEntry[]>({
    // Refetch when filter changes; the backend already does an ILIKE match,
    // so a prefix string filters by prefix.
    queryKey: ['audit-log', effectiveAction, limit],
    queryFn: () => auditApi.list({ action: effectiveAction, limit }),
  });

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <ScrollText size={22} /> Audit log
      </h1>
      <p style={{ color: 'var(--color-text-secondary, #64748b)', marginTop: 0 }}>
        Read-only view of every recorded admin/auth/billing event. Newest
        first; use the filters to scope down.
      </p>

      <div style={{ display: 'flex', gap: 12, alignItems: 'flex-end', flexWrap: 'wrap', marginBottom: 16 }}>
        <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
          <span style={{ marginBottom: 4 }}>Action prefix</span>
          <select
            value={prefix}
            onChange={(e) => { setPrefix(e.target.value); setActionFilter(''); }}
            style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }}
          >
            {PREFIXES.map((p) => (
              <option key={p.value} value={p.value}>{p.label}</option>
            ))}
          </select>
        </label>
        <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
          <span style={{ marginBottom: 4 }}>Action filter (overrides prefix)</span>
          <input
            type="text"
            value={actionFilter}
            onChange={(e) => setActionFilter(e.target.value)}
            placeholder="e.g. auth.login"
            style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)', minWidth: 200 }}
          />
        </label>
        <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
          <span style={{ marginBottom: 4 }}>Limit</span>
          <select
            value={limit}
            onChange={(e) => setLimit(parseInt(e.target.value, 10))}
            style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }}
          >
            {[25, 50, 100, 250, 500].map((n) => (
              <option key={n} value={n}>{n}</option>
            ))}
          </select>
        </label>
        <button
          className="btn btn--ghost"
          onClick={() => query.refetch()}
          disabled={query.isFetching}
          title="Refresh"
        >
          <RefreshCw size={16} className={query.isFetching ? 'spin' : ''} /> Refresh
        </button>
      </div>

      {query.isLoading && <p>Loading audit log…</p>}
      {query.isError && (
        <div role="alert" style={{ color: 'var(--color-danger, #dc2626)' }}>
          Couldn't load audit log: {(query.error as Error)?.message ?? 'unknown error'}
        </div>
      )}
      {query.data && (
        <>
          <p style={{ fontSize: 12, color: 'var(--color-text-secondary, #64748b)' }}>
            {query.data.length} {query.data.length === 1 ? 'entry' : 'entries'}
          </p>
          <table className="audit-table" style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
                <th style={{ textAlign: 'left', padding: '8px 12px', whiteSpace: 'nowrap' }}>When</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Action</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Resource</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Actor</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>IP</th>
                <th style={{ textAlign: 'left', padding: '8px 12px' }}>Details</th>
              </tr>
            </thead>
            <tbody>
              {query.data.length === 0 && (
                <tr>
                  <td colSpan={6} style={{ padding: 24, textAlign: 'center', color: 'var(--color-text-secondary, #64748b)' }}>
                    No audit-log entries match the current filter.
                  </td>
                </tr>
              )}
              {query.data.map((row) => (
                <tr key={row.id} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                  <td style={{ padding: '8px 12px', whiteSpace: 'nowrap' }}>{formatDate(row.created_at)}</td>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace' }}>{row.action}</td>
                  <td style={{ padding: '8px 12px' }}>
                    {row.resource_type ?? '—'}
                    {row.resource_id ? <span style={{ color: 'var(--color-text-secondary, #64748b)' }}> · {row.resource_id.slice(0, 8)}</span> : null}
                  </td>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: 12 }}>
                    {row.mailbox_id ? row.mailbox_id.slice(0, 8) : '—'}
                  </td>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: 12 }}>{row.ip_address ?? '—'}</td>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: 12, maxWidth: 320, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={row.details ? JSON.stringify(row.details) : ''}>
                    {row.details ? JSON.stringify(row.details) : '—'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
