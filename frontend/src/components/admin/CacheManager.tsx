// TMAIL-199: cache admin page.
//
// Three sections: connection + TTL config (status), Redis INFO (stats),
// and a destructive Flush all button gated by a confirm modal. Uses the
// same TanStack Query pattern as the rest of the admin pages.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Database, RefreshCw, Trash2 } from 'lucide-react';
import { cacheApi, type CacheStatus, type CacheStatsResponse } from '../../api/cache';

function formatSeconds(s: number): string {
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.round(s / 60)}m`;
  return `${Math.round(s / 3600)}h`;
}

export function CacheManager() {
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState(false);
  const [flushResult, setFlushResult] = useState<string | null>(null);

  const status = useQuery<CacheStatus>({
    queryKey: ['admin-cache-status'],
    queryFn: () => cacheApi.status(),
  });
  const stats = useQuery<CacheStatsResponse>({
    queryKey: ['admin-cache-stats'],
    queryFn: () => cacheApi.stats(),
  });

  const flushMut = useMutation({
    mutationFn: () => cacheApi.flush(),
    onSuccess: (res) => {
      setFlushResult(res.message);
      setConfirming(false);
      queryClient.invalidateQueries({ queryKey: ['admin-cache-stats'] });
    },
    onError: (err: Error) => {
      setFlushResult(`Flush failed: ${err.message}`);
    },
  });

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <Database size={22} /> Cache
      </h1>

      <section style={{ marginBottom: 24 }}>
        <h2 style={{ fontSize: 15, marginBottom: 8 }}>Connection</h2>
        {status.isLoading && <p>Loading…</p>}
        {status.isError && (
          <p style={{ color: 'var(--color-danger, #dc2626)' }}>
            {(status.error as Error)?.message ?? 'unknown error'}
          </p>
        )}
        {status.data && (
          <dl style={{ display: 'grid', gridTemplateColumns: 'minmax(180px, max-content) 1fr', gap: '6px 16px', fontSize: 13 }}>
            <dt>Status</dt>
            <dd>
              <span
                style={{
                  display: 'inline-block',
                  width: 10,
                  height: 10,
                  borderRadius: 5,
                  background: status.data.connected ? '#22c55e' : '#dc2626',
                  marginRight: 6,
                }}
              />
              {status.data.connected ? 'Connected' : 'Disconnected'}
            </dd>
            <dt>Redis URL</dt>
            <dd style={{ fontFamily: 'monospace' }}>{status.data.redis_url}</dd>
            <dt>Branding TTL</dt>
            <dd>{formatSeconds(status.data.branding_ttl_secs)}</dd>
            <dt>Quota TTL</dt>
            <dd>{formatSeconds(status.data.quota_ttl_secs)}</dd>
            <dt>Session TTL</dt>
            <dd>{formatSeconds(status.data.session_ttl_secs)}</dd>
            <dt>Rate limit</dt>
            <dd>{status.data.rate_limit_max_requests} requests / {formatSeconds(status.data.rate_limit_window_secs)}</dd>
          </dl>
        )}
      </section>

      <section style={{ marginBottom: 24 }}>
        <h2 style={{ fontSize: 15, marginBottom: 8, display: 'flex', alignItems: 'center', gap: 8 }}>
          Redis INFO
          <button
            className="btn btn--ghost btn--sm"
            onClick={() => stats.refetch()}
            disabled={stats.isFetching}
            title="Refresh"
          >
            <RefreshCw size={14} />
          </button>
        </h2>
        {stats.isLoading && <p>Loading…</p>}
        {stats.data && stats.data.connected && stats.data.info && (
          <pre
            style={{
              background: 'var(--color-bg-elevated, #f8fafc)',
              border: '1px solid var(--color-border, #e5e7eb)',
              padding: 12,
              borderRadius: 6,
              maxHeight: 280,
              overflow: 'auto',
              fontSize: 12,
            }}
          >
            {stats.data.info}
          </pre>
        )}
        {stats.data && !stats.data.connected && (
          <p style={{ color: 'var(--color-text-secondary, #64748b)' }}>Redis is not reachable.</p>
        )}
      </section>

      <section>
        <h2 style={{ fontSize: 15, marginBottom: 8 }}>Destructive actions</h2>
        {!confirming && (
          <button
            className="btn btn--danger"
            onClick={() => setConfirming(true)}
            disabled={flushMut.isPending || !status.data?.connected}
          >
            <Trash2 size={14} /> Flush all cache keys
          </button>
        )}
        {confirming && (
          <div
            role="alertdialog"
            aria-labelledby="flush-confirm-title"
            style={{
              border: '1px solid var(--color-danger, #dc2626)',
              padding: 16,
              borderRadius: 8,
              background: 'rgba(220, 38, 38, 0.05)',
              maxWidth: 480,
            }}
          >
            <p id="flush-confirm-title" style={{ marginTop: 0, fontWeight: 600 }}>
              Flush every cached value?
            </p>
            <p style={{ fontSize: 13, color: 'var(--color-text-secondary, #64748b)' }}>
              Quota lookups, branding, and rate-limit counters will repopulate on next request.
              Sessions stored in Redis will be invalidated immediately.
            </p>
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                className="btn btn--danger"
                onClick={() => flushMut.mutate()}
                disabled={flushMut.isPending}
              >
                {flushMut.isPending ? 'Flushing…' : 'Yes, flush'}
              </button>
              <button className="btn btn--ghost" onClick={() => setConfirming(false)} disabled={flushMut.isPending}>
                Cancel
              </button>
            </div>
          </div>
        )}
        {flushResult && (
          <p
            role="status"
            style={{
              marginTop: 12,
              padding: '8px 12px',
              background: 'var(--color-bg-elevated, #f8fafc)',
              border: '1px solid var(--color-border, #e5e7eb)',
              borderRadius: 6,
              fontSize: 13,
            }}
          >
            {flushResult}
          </p>
        )}
      </section>
    </div>
  );
}
