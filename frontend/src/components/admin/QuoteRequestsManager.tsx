// TMAIL-185: admin dashboard for the enterprise_quote_requests inbox.
//
// Three columns:
//   1. Status filter sidebar with live counts (from /admin/quote-requests/stats)
//   2. Paginated list of requests in the selected status
//   3. Detail panel for the focused row with state-transition buttons
//
// Mirrors the visual language of FeatureFlagsManager and the standard admin
// chrome. Lives behind RequireAuth — role gating to follow once roles ship.

import { useEffect, useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Mail, RefreshCw, ChevronRight } from 'lucide-react';
import { quoteRequestsApi, type QuoteRequest, type QuoteStatus, type StatusCount } from '../../api/quoteRequests';
import './QuoteRequestsManager.css';

const ALL_STATUSES: QuoteStatus[] = ['new', 'contacted', 'quoted', 'won', 'lost'];

const STATUS_LABEL: Record<QuoteStatus, string> = {
  new: 'New',
  contacted: 'Contacted',
  quoted: 'Quoted',
  won: 'Won',
  lost: 'Lost',
};

const STATUS_COLOR: Record<QuoteStatus, string> = {
  new: '#2563eb',
  contacted: '#f59e0b',
  quoted: '#8b5cf6',
  won: '#16a34a',
  lost: '#94a3b8',
};

export function QuoteRequestsManager() {
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<QuoteStatus | 'all'>('new');
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const stats = useQuery<StatusCount[]>({
    queryKey: ['admin', 'quote-requests', 'stats'],
    queryFn: quoteRequestsApi.stats,
    staleTime: 30_000,
  });

  const list = useQuery({
    queryKey: ['admin', 'quote-requests', 'list', status],
    queryFn: () => quoteRequestsApi.list(status === 'all' ? undefined : status, 50, 0),
    staleTime: 15_000,
  });

  // Auto-select the first row when the list loads or the filter changes.
  useEffect(() => {
    if (list.data?.items.length && !list.data.items.find((i) => i.id === selectedId)) {
      setSelectedId(list.data.items[0].id);
    } else if (list.data?.items.length === 0) {
      setSelectedId(null);
    }
  }, [list.data, selectedId]);

  const detail = useQuery<QuoteRequest>({
    queryKey: ['admin', 'quote-requests', 'detail', selectedId],
    queryFn: () => quoteRequestsApi.get(selectedId as string),
    enabled: !!selectedId,
    staleTime: 15_000,
  });

  const transition = useMutation({
    mutationFn: ({ id, body }: { id: string; body: { status?: QuoteStatus; internal_notes?: string } }) =>
      quoteRequestsApi.update(id, body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin', 'quote-requests'] });
    },
  });

  const totalsByStatus = useMemo(() => {
    const m = new Map<QuoteStatus, number>();
    for (const s of stats.data ?? []) m.set(s.status, s.count);
    return m;
  }, [stats.data]);

  const totalAll = useMemo(
    () => Array.from(totalsByStatus.values()).reduce((a, b) => a + b, 0),
    [totalsByStatus]
  );

  return (
    <div className="qrm">
      <header className="qrm__header">
        <div>
          <h1>Enterprise quote requests</h1>
          <p>Triaging inbox for the public quote form on the landing page. Walk each lead through new → contacted → quoted → won/lost.</p>
        </div>
        <button className="qrm-btn qrm-btn--ghost" onClick={() => { stats.refetch(); list.refetch(); }} title="Refresh">
          <RefreshCw size={16} />
        </button>
      </header>

      <div className="qrm__layout">
        {/* ---- column 1: status filter ---- */}
        <aside className="qrm-filters">
          <button
            className={`qrm-filter ${status === 'all' ? 'is-active' : ''}`}
            onClick={() => setStatus('all')}
          >
            <span>All</span>
            <span className="qrm-filter__count">{totalAll}</span>
          </button>
          {ALL_STATUSES.map((s) => (
            <button
              key={s}
              className={`qrm-filter ${status === s ? 'is-active' : ''}`}
              onClick={() => setStatus(s)}
            >
              <span>
                <span className="qrm-status-dot" style={{ background: STATUS_COLOR[s] }} />
                {STATUS_LABEL[s]}
              </span>
              <span className="qrm-filter__count">{totalsByStatus.get(s) ?? 0}</span>
            </button>
          ))}
        </aside>

        {/* ---- column 2: list ---- */}
        <ul className="qrm-list">
          {list.isLoading && <li className="qrm-empty">Loading…</li>}
          {!list.isLoading && (list.data?.items.length ?? 0) === 0 && (
            <li className="qrm-empty">
              <Mail size={28} />
              <p>No quote requests {status !== 'all' ? `with status ${STATUS_LABEL[status]}` : 'yet'}.</p>
            </li>
          )}
          {list.data?.items.map((item) => (
            <li
              key={item.id}
              className={`qrm-row ${selectedId === item.id ? 'is-selected' : ''}`}
              onClick={() => setSelectedId(item.id)}
            >
              <div className="qrm-row__main">
                <div className="qrm-row__name">{item.contact_name}</div>
                <div className="qrm-row__meta">
                  <span>{item.contact_email}</span>
                  {item.company && <span> · {item.company}</span>}
                  {item.estimated_users != null && <span> · {item.estimated_users} users</span>}
                </div>
                <div className="qrm-row__msg">{item.message}</div>
              </div>
              <div className="qrm-row__side">
                <span className="qrm-status" style={{ borderColor: STATUS_COLOR[item.status], color: STATUS_COLOR[item.status] }}>
                  {STATUS_LABEL[item.status]}
                </span>
                <span className="qrm-row__date">{item.created_at ? new Date(item.created_at).toLocaleDateString() : ''}</span>
                <ChevronRight size={16} className="qrm-row__chevron" />
              </div>
            </li>
          ))}
        </ul>

        {/* ---- column 3: detail + transitions ---- */}
        <section className="qrm-detail">
          {!selectedId && <p className="qrm-detail__empty">Select a quote request to inspect.</p>}
          {selectedId && detail.data && (
            <DetailPanel
              quote={detail.data}
              onTransition={(s, notes) =>
                transition.mutate({ id: selectedId, body: { status: s, internal_notes: notes } })
              }
              busy={transition.isPending}
            />
          )}
        </section>
      </div>
    </div>
  );
}

function DetailPanel({
  quote,
  onTransition,
  busy,
}: {
  quote: QuoteRequest;
  onTransition: (status: QuoteStatus, notes?: string) => void;
  busy: boolean;
}) {
  const [notes, setNotes] = useState(quote.internal_notes ?? '');
  // Reset the notes textarea whenever the user picks a different row.
  useEffect(() => setNotes(quote.internal_notes ?? ''), [quote.id, quote.internal_notes]);

  return (
    <div className="qrm-detail__inner">
      <div className="qrm-detail__head">
        <div>
          <h2>{quote.contact_name}</h2>
          <p>{quote.contact_email}{quote.company ? ` · ${quote.company}` : ''}</p>
        </div>
        <span
          className="qrm-status"
          style={{ borderColor: STATUS_COLOR[quote.status], color: STATUS_COLOR[quote.status] }}
        >
          {STATUS_LABEL[quote.status]}
        </span>
      </div>

      <dl className="qrm-detail__grid">
        <dt>Estimated users</dt><dd>{quote.estimated_users ?? '—'}</dd>
        <dt>Tracking id</dt><dd><code>{quote.id.slice(0, 8)}</code></dd>
        <dt>Submitted</dt><dd>{quote.created_at ? new Date(quote.created_at).toLocaleString() : '—'}</dd>
        <dt>Updated</dt><dd>{quote.updated_at ? new Date(quote.updated_at).toLocaleString() : '—'}</dd>
        {quote.contacted_at && (<><dt>Contacted</dt><dd>{new Date(quote.contacted_at).toLocaleString()}</dd></>)}
        {quote.quoted_at && (<><dt>Quoted</dt><dd>{new Date(quote.quoted_at).toLocaleString()}</dd></>)}
        {quote.closed_at && (<><dt>Closed</dt><dd>{new Date(quote.closed_at).toLocaleString()}</dd></>)}
      </dl>

      <div className="qrm-message">
        <h3>Message</h3>
        <pre>{quote.message}</pre>
      </div>

      <div className="qrm-notes">
        <h3>Internal notes</h3>
        <textarea
          rows={5}
          placeholder="Add private notes for the sales team — not shown to the customer."
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />
      </div>

      <div className="qrm-actions">
        <span className="qrm-actions__label">Transition →</span>
        {ALL_STATUSES.filter((s) => s !== quote.status).map((s) => (
          <button
            key={s}
            className="qrm-btn"
            disabled={busy}
            onClick={() => onTransition(s, notes !== (quote.internal_notes ?? '') ? notes : undefined)}
            style={{ background: STATUS_COLOR[s], color: 'white' }}
          >
            Mark {STATUS_LABEL[s]}
          </button>
        ))}
        {notes !== (quote.internal_notes ?? '') && (
          <button
            className="qrm-btn qrm-btn--ghost"
            disabled={busy}
            onClick={() => onTransition(quote.status, notes)}
          >
            Save notes only
          </button>
        )}
      </div>
    </div>
  );
}
