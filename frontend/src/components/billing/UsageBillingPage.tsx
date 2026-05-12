// TMAIL-179: Usage & billing page for the in-app sidebar.
//
// Three sections, top to bottom:
//   1. Hero card with current storage + projected end-of-month bill
//   2. Period stats (avg, peak, sample count, daily trend interpretation)
//   3. Invoice history table
//
// Pulls from /api/billing/usage and /api/billing/invoices. Shows a USD
// equivalent next to GHS amounts when the visitor's locale is not en-GH.

import { useQuery } from '@tanstack/react-query';
import { RefreshCw, FileText, Receipt, Database } from 'lucide-react';
import { usageBillingApi, type UsageResponse, type UsageInvoiceRow } from '../../api/billing';
import './UsageBillingPage.css';

const GHS_TO_USD = 0.067; // matches LandingPage's GHS_TO_USD_RATE
const GH_LOCALE_RE = /(^en-GH$|^en-GH-|-GH$|-gh$|^ak\b|^tw\b|^ee\b|^ga\b|-Gh-)/i;
function isGhanaLocale() {
  if (typeof navigator === 'undefined') return true;
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
  return langs.some((l) => GH_LOCALE_RE.test(l));
}
function formatGhs(n: number): string {
  return new Intl.NumberFormat('en-GH', { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(n);
}
function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(units.length - 1, Math.floor(Math.log10(bytes) / 3));
  return `${(bytes / Math.pow(1000, i)).toFixed(i === 0 ? 0 : 2)} ${units[i]}`;
}

export function UsageBillingPage() {
  const usage = useQuery<UsageResponse>({
    queryKey: ['billing', 'usage'],
    queryFn: usageBillingApi.usage,
    staleTime: 30_000,
  });
  const invoices = useQuery<UsageInvoiceRow[]>({
    queryKey: ['billing', 'invoices'],
    queryFn: usageBillingApi.invoices,
    staleTime: 60_000,
  });

  const showUsd = !isGhanaLocale();

  return (
    <div className="ubp">
      <header className="ubp__header">
        <div>
          <h1>Usage & billing</h1>
          <p>Pay only for what you store. Your monthly bill is computed nightly from the average of every storage snapshot taken this month.</p>
        </div>
        <button
          className="ubp-btn ubp-btn--ghost"
          onClick={() => { usage.refetch(); invoices.refetch(); }}
          title="Refresh"
        ><RefreshCw size={16} /></button>
      </header>

      {usage.isLoading && <div className="ubp__loading">Loading usage…</div>}
      {usage.error && <div className="ubp__error">Could not load usage. {(usage.error as Error).message}</div>}

      {usage.data && <UsageHero u={usage.data} showUsd={showUsd} />}
      {usage.data && <UsageStats u={usage.data} />}

      <section className="ubp-invoices">
        <h2><Receipt size={18} /> Invoice history</h2>
        {invoices.isLoading && <div className="ubp__loading">Loading invoices…</div>}
        {invoices.data && invoices.data.length === 0 && (
          <div className="ubp-invoices__empty">
            <FileText size={28} />
            <p>No invoices yet. Your first one will appear after the current period closes.</p>
          </div>
        )}
        {invoices.data && invoices.data.length > 0 && (
          <table className="ubp-invoices__table">
            <thead>
              <tr>
                <th>Period</th>
                <th>Avg storage</th>
                <th>Amount</th>
                <th>Status</th>
                <th>Paid</th>
              </tr>
            </thead>
            <tbody>
              {invoices.data.map((inv) => (
                <tr key={inv.id}>
                  <td>{inv.period_start} → {inv.period_end}</td>
                  <td>{formatBytes(inv.avg_storage_bytes)}</td>
                  <td>
                    GHS {formatGhs(inv.amount_ghs)}
                    {inv.minimum_applied && <span className="ubp-min-tag" title="Monthly minimum applied">min</span>}
                  </td>
                  <td><span className={`ubp-status ubp-status--${inv.status}`}>{inv.status}</span></td>
                  <td>{inv.paid_at ? new Date(inv.paid_at).toLocaleDateString() : '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}

function UsageHero({ u, showUsd }: { u: UsageResponse; showUsd: boolean }) {
  return (
    <section className="ubp-hero">
      <div className="ubp-hero__left">
        <span className="ubp-hero__label">Projected charge for {monthLabel(u.period_start)}</span>
        <div className="ubp-hero__price">
          GHS {formatGhs(u.projected_amount_ghs)}
          {showUsd && <span className="ubp-hero__usd"> (≈ ${(u.projected_amount_ghs * GHS_TO_USD).toFixed(2)} USD)</span>}
        </div>
        <p className="ubp-hero__sub">
          Based on {u.projected_billed_gb} billed GB at GHS {u.ghs_per_gb}/GB.
          {u.projected_minimum_applied && (
            <> The GHS {u.ghs_monthly_min} monthly minimum was applied.</>
          )}
        </p>
      </div>
      <div className="ubp-hero__right">
        <Database size={18} />
        <div>
          <div className="ubp-hero__current">{formatBytes(u.current_storage_bytes)} stored right now</div>
          <div className="ubp-hero__sub">Refreshed nightly from your IMAP server</div>
        </div>
      </div>
    </section>
  );
}

function UsageStats({ u }: { u: UsageResponse }) {
  return (
    <section className="ubp-stats">
      <Stat label="This month, average" value={formatBytes(u.avg_storage_bytes)} />
      <Stat label="This month, peak" value={formatBytes(u.peak_storage_bytes)} />
      <Stat label="Snapshots so far" value={String(u.sample_count)} />
    </section>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="ubp-stat">
      <div className="ubp-stat__value">{value}</div>
      <div className="ubp-stat__label">{label}</div>
    </div>
  );
}

function monthLabel(isoDate: string): string {
  const d = new Date(isoDate);
  return d.toLocaleString(undefined, { month: 'long', year: 'numeric' });
}
