// TMAIL-352: Modern UI viewer for the admin audit log.
//
// Filters: user (mailbox dropdown, populated from /api/admin/users), action
// (prefix dropdown + free-form override, mirroring the classic SPA), date
// range (from / to local-datetime inputs). Paginated 50 rows at a time with
// prev/next driven by the `X-Total-Count` response header.
import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { RefreshCw, ScrollText, ChevronLeft, ChevronRight } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import {
  adminAuditLogApi,
  type AuditLogPage,
} from '@/api/admin-audit-log';
import { adminUsersApi, type UserInfo } from '@/api/admin-users';
import { localToIso } from './audit-log-helpers';

// Keep in sync with the classic SPA's PREFIXES table (TMAIL-198) — trailing
// dot tells the backend "prefix match" (auth.* → action LIKE 'auth.%').
const PREFIXES: { label: string; value: string }[] = [
  { label: 'All', value: '' },
  { label: 'auth.*', value: 'auth.' },
  { label: 'billing.*', value: 'billing.' },
  { label: 'admin.*', value: 'admin.' },
  { label: 'domain.*', value: 'domain.' },
  { label: 'webhook.*', value: 'webhook.' },
];

const PAGE_SIZE = 50;

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function AuditLogTab() {
  const [mailboxId, setMailboxId] = useState<string>('');
  const [prefix, setPrefix] = useState<string>('');
  const [actionOverride, setActionOverride] = useState<string>('');
  const [from, setFrom] = useState<string>('');
  const [to, setTo] = useState<string>('');
  const [page, setPage] = useState<number>(0);

  // For the mailbox dropdown — reuse the same /api/admin/users data the
  // Mailboxes panel loads so we don't double-fetch.
  const usersQ = useQuery<UserInfo[]>({
    queryKey: ['admin', 'users'],
    queryFn: () => adminUsersApi.list(),
  });

  const effectiveAction = actionOverride.trim() || prefix || undefined;
  const fromIso = localToIso(from);
  const toIso = localToIso(to);

  const auditQ = useQuery<AuditLogPage>({
    queryKey: [
      'admin',
      'audit-log',
      mailboxId || null,
      effectiveAction ?? null,
      fromIso ?? null,
      toIso ?? null,
      page,
    ],
    queryFn: () =>
      adminAuditLogApi.listPaginated({
        mailbox_id: mailboxId || undefined,
        action: effectiveAction,
        from: fromIso,
        to: toIso,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      }),
  });

  // username@domain lookup so the table can render readable actors.
  const userLabelById = useMemo(() => {
    const map = new Map<string, string>();
    (usersQ.data ?? []).forEach((u) => {
      map.set(u.id, u.display_name?.trim() || u.username);
    });
    return map;
  }, [usersQ.data]);

  const total = auditQ.data?.total ?? 0;
  const entries = auditQ.data?.entries ?? [];
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const start = total === 0 ? 0 : page * PAGE_SIZE + 1;
  const end = total === 0 ? 0 : Math.min(total, page * PAGE_SIZE + entries.length);

  const resetToFirstPage = () => setPage(0);

  return (
    <div className="space-y-4" data-testid="audit-log-tab">
      <Card className="p-4 space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <ScrollText className="size-5" /> Audit log
          </h2>
          <Button
            variant="outline"
            size="sm"
            onClick={() => auditQ.refetch()}
            disabled={auditQ.isFetching}
            data-testid="audit-refresh"
          >
            <RefreshCw className={`size-4 mr-1 ${auditQ.isFetching ? 'animate-spin' : ''}`} />
            Refresh
          </Button>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-3">
          <label className="flex flex-col text-xs gap-1">
            <span>User</span>
            <select
              data-testid="audit-filter-user"
              className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-2 text-sm"
              value={mailboxId}
              onChange={(e) => {
                setMailboxId(e.target.value);
                resetToFirstPage();
              }}
            >
              <option value="">All users</option>
              {(usersQ.data ?? []).map((u) => (
                <option key={u.id} value={u.id}>
                  {u.display_name?.trim() || u.username}
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-col text-xs gap-1">
            <span>Action prefix</span>
            <select
              data-testid="audit-filter-prefix"
              className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-2 text-sm"
              value={prefix}
              onChange={(e) => {
                setPrefix(e.target.value);
                setActionOverride('');
                resetToFirstPage();
              }}
            >
              {PREFIXES.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
          </label>

          <label className="flex flex-col text-xs gap-1">
            <span>Action (overrides prefix)</span>
            <Input
              data-testid="audit-filter-action"
              type="text"
              value={actionOverride}
              placeholder="e.g. auth.login"
              onChange={(e) => {
                setActionOverride(e.target.value);
                resetToFirstPage();
              }}
            />
          </label>

          <label className="flex flex-col text-xs gap-1">
            <span>From</span>
            <Input
              data-testid="audit-filter-from"
              type="datetime-local"
              value={from}
              onChange={(e) => {
                setFrom(e.target.value);
                resetToFirstPage();
              }}
            />
          </label>

          <label className="flex flex-col text-xs gap-1">
            <span>To</span>
            <Input
              data-testid="audit-filter-to"
              type="datetime-local"
              value={to}
              onChange={(e) => {
                setTo(e.target.value);
                resetToFirstPage();
              }}
            />
          </label>
        </div>
      </Card>

      <Card className="p-0 overflow-hidden">
        <div className="overflow-x-auto">
          {auditQ.isLoading ? (
            <div className="p-6 text-zinc-500 text-sm">Loading audit log…</div>
          ) : auditQ.isError ? (
            <div className="p-6 text-red-600 text-sm" data-testid="audit-error">
              Couldn't load audit log: {(auditQ.error as Error)?.message ?? 'unknown error'}
            </div>
          ) : entries.length === 0 ? (
            <div className="p-6 text-zinc-500 text-sm" data-testid="audit-empty">
              No audit log entries match the current filter.
            </div>
          ) : (
            <table className="w-full text-sm" data-testid="audit-table">
              <thead className="bg-zinc-50 dark:bg-zinc-900">
                <tr>
                  <th className="text-left py-2 px-3 font-medium whitespace-nowrap">When</th>
                  <th className="text-left py-2 px-3 font-medium">Action</th>
                  <th className="text-left py-2 px-3 font-medium">Resource</th>
                  <th className="text-left py-2 px-3 font-medium">Actor</th>
                  <th className="text-left py-2 px-3 font-medium">IP</th>
                  <th className="text-left py-2 px-3 font-medium">Details</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((row) => {
                  const actor = row.mailbox_id
                    ? userLabelById.get(row.mailbox_id) ?? row.mailbox_id.slice(0, 8)
                    : '—';
                  return (
                    <tr
                      key={row.id}
                      className="border-t border-zinc-200 dark:border-zinc-800"
                      data-testid="audit-row"
                    >
                      <td className="py-2 px-3 whitespace-nowrap text-zinc-600 dark:text-zinc-400">
                        {formatDate(row.created_at)}
                      </td>
                      <td className="py-2 px-3 font-mono text-xs">{row.action}</td>
                      <td className="py-2 px-3">
                        {row.resource_type ?? '—'}
                        {row.resource_id ? (
                          <span className="text-zinc-500 text-xs"> · {row.resource_id.slice(0, 8)}</span>
                        ) : null}
                      </td>
                      <td className="py-2 px-3 font-mono text-xs">{actor}</td>
                      <td className="py-2 px-3 font-mono text-xs">{row.ip_address ?? '—'}</td>
                      <td
                        className="py-2 px-3 font-mono text-xs max-w-[320px] truncate"
                        title={row.details ? JSON.stringify(row.details) : ''}
                      >
                        {row.details ? JSON.stringify(row.details) : '—'}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>

        <div className="flex items-center justify-between px-4 py-3 border-t border-zinc-200 dark:border-zinc-800 text-sm">
          <div className="text-zinc-600 dark:text-zinc-400" data-testid="audit-pagination-status">
            {total === 0 ? 'No entries' : `Showing ${start}–${end} of ${total}`}
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={page === 0 || auditQ.isFetching}
              onClick={() => setPage((p) => Math.max(0, p - 1))}
              data-testid="audit-prev"
            >
              <ChevronLeft className="size-4 mr-1" /> Prev
            </Button>
            <span className="text-zinc-500" data-testid="audit-page-label">
              Page {page + 1} of {totalPages}
            </span>
            <Button
              variant="outline"
              size="sm"
              disabled={page + 1 >= totalPages || auditQ.isFetching}
              onClick={() => setPage((p) => p + 1)}
              data-testid="audit-next"
            >
              Next <ChevronRight className="size-4 ml-1" />
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
