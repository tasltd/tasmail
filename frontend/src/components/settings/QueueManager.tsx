// Added: Email queue management settings panel for TMAIL-58
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, RefreshCw, Trash2, RotateCcw } from 'lucide-react';
import {
  fetchQueueItems,
  fetchQueueStats,
  cancelQueueItem,
  retryQueueItem,
} from '../../api/queue';
import type { EmailQueueItem, QueueStats } from '../../api/queue';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Badge component showing status with appropriate color
 * CONSTRAINTS: Only renders known queue statuses
 */
function StatusBadge({ status }: { status: EmailQueueItem['status'] }) {
  // Added: Color mapping for each queue status
  const statusColors: Record<string, string> = {
    pending: '#2196f3',
    sending: '#ff9800',
    sent: '#4caf50',
    failed: '#f44336',
    dead_letter: '#9c27b0',
  };

  const displayLabel = status === 'dead_letter' ? 'Dead Letter' : status.charAt(0).toUpperCase() + status.slice(1);

  return (
    <span
      style={{
        fontSize: '11px',
        background: statusColors[status] || '#888',
        color: 'white',
        padding: '2px 8px',
        borderRadius: '10px',
        whiteSpace: 'nowrap',
      }}
    >
      {displayLabel}
    </span>
  );
}

/**
 * PURPOSE: Display aggregate queue stats as a compact summary bar
 */
function QueueStatsBar({ stats }: { stats: QueueStats }) {
  return (
    <div
      style={{
        display: 'flex',
        gap: '16px',
        padding: '12px 16px',
        background: 'var(--color-bg-secondary, #f5f5f5)',
        borderRadius: '8px',
        fontSize: '13px',
        flexWrap: 'wrap',
      }}
    >
      <span>Pending: <strong>{stats.pending}</strong></span>
      <span>Sending: <strong>{stats.sending}</strong></span>
      <span>Sent: <strong>{stats.sent}</strong></span>
      <span>Failed: <strong>{stats.failed}</strong></span>
      <span>Dead Letter: <strong>{stats.dead_letter}</strong></span>
    </div>
  );
}

/**
 * PURPOSE: Email queue management panel — view, cancel, and retry queued emails
 * EXTERNAL: Uses TanStack Query for data fetching and cache invalidation
 */
export function QueueManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [statusFilter, setStatusFilter] = useState<string | undefined>(undefined);

  // Added: Fetch queue stats for the summary bar
  const { data: stats } = useQuery({
    queryKey: ['queue-stats'],
    queryFn: fetchQueueStats,
    refetchInterval: 10000, // NOTE: Auto-refresh every 10s for live updates
  });

  // Added: Fetch queue items with optional status filter
  const { data: queueItems, isLoading } = useQuery({
    queryKey: ['queue-items', statusFilter],
    queryFn: () => fetchQueueItems(statusFilter),
    refetchInterval: 10000,
  });

  // Added: Cancel mutation with cache invalidation
  const cancelMutation = useMutation({
    mutationFn: cancelQueueItem,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['queue-items'] });
      queryClient.invalidateQueries({ queryKey: ['queue-stats'] });
    },
  });

  // Added: Retry mutation with cache invalidation
  const retryMutation = useMutation({
    mutationFn: retryQueueItem,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['queue-items'] });
      queryClient.invalidateQueries({ queryKey: ['queue-stats'] });
    },
  });

  if (isLoading) return <LoadingSkeleton rows={5} />;

  return (
    <div className="queue-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Email Queue</h2>
        <button
          className="btn"
          onClick={() => {
            queryClient.invalidateQueries({ queryKey: ['queue-items'] });
            queryClient.invalidateQueries({ queryKey: ['queue-stats'] });
          }}
          title="Refresh"
        >
          <RefreshCw size={16} /> Refresh
        </button>
      </div>

      {/* Added: Queue statistics summary */}
      {stats && (
        <div style={{ marginTop: '12px' }}>
          <QueueStatsBar stats={stats} />
        </div>
      )}

      {/* Added: Status filter buttons */}
      <div style={{ marginTop: '12px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
        {[undefined, 'pending', 'sending', 'sent', 'failed', 'dead_letter'].map((filterValue) => (
          <button
            key={filterValue ?? 'all'}
            className={`btn ${statusFilter === filterValue ? 'btn--primary' : ''}`}
            onClick={() => setStatusFilter(filterValue)}
            style={{ fontSize: '12px', padding: '4px 12px' }}
          >
            {filterValue ? (filterValue === 'dead_letter' ? 'Dead Letter' : filterValue.charAt(0).toUpperCase() + filterValue.slice(1)) : 'All'}
          </button>
        ))}
      </div>

      {/* Added: Queue items list */}
      <div style={{ marginTop: '16px' }}>
        {(!queueItems || queueItems.length === 0) && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No queued emails{statusFilter ? ` with status "${statusFilter}"` : ''}.
          </p>
        )}
        {queueItems?.map((item) => (
          <div
            key={item.id}
            style={{
              display: 'flex',
              alignItems: 'flex-start',
              gap: '12px',
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <strong style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {item.subject || '(No Subject)'}
                </strong>
                <StatusBadge status={item.status} />
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '4px' }}>
                To: {item.to_addresses.join(', ')}
              </div>
              {item.retry_count > 0 && (
                <div style={{ fontSize: '11px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  Retries: {item.retry_count}/{item.max_retries}
                </div>
              )}
              {item.last_error && (
                <div style={{ fontSize: '11px', color: '#f44336', marginTop: '2px' }}>
                  Error: {item.last_error.slice(0, 120)}{item.last_error.length > 120 ? '...' : ''}
                </div>
              )}
              <div style={{ fontSize: '11px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                Created: {new Date(item.created_at).toLocaleString()}
              </div>
            </div>
            {/* Added: Action buttons — retry for failed/dead_letter, cancel for non-sending */}
            <div style={{ display: 'flex', gap: '4px', flexShrink: 0 }}>
              {(item.status === 'failed' || item.status === 'dead_letter') && (
                <button
                  className="btn btn--icon"
                  onClick={() => retryMutation.mutate(item.id)}
                  title="Retry"
                  disabled={retryMutation.isPending}
                >
                  <RotateCcw size={16} />
                </button>
              )}
              {item.status !== 'sending' && item.status !== 'sent' && (
                <button
                  className="btn btn--icon btn--danger"
                  onClick={() => cancelMutation.mutate(item.id)}
                  title="Cancel"
                  disabled={cancelMutation.isPending}
                >
                  <Trash2 size={16} />
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
