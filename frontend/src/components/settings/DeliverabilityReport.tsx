// Added: Deliverability report UI component for TMAIL-39
// PURPOSE: Admin tool to run and display email deliverability checks with scored results
// EXTERNAL: Uses TanStack Query for data fetching, lucide-react for icons

import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import {
  ArrowLeft,
  CheckCircle,
  XCircle,
  AlertTriangle,
  AlertOctagon,
  Play,
  RefreshCw,
  Copy,
  ExternalLink,
  Mail,
} from 'lucide-react';
import {
  runDeliverabilityCheck,
  getExternalDeliverabilityTools,
} from '../../api/deliverability';
import type {
  CheckResult,
  CheckStatus,
  ExternalToolsResponse,
} from '../../api/deliverability';
import { useMailStore } from '../../stores/mailStore';

// Added: Color mapping for check status indicators
const STATUS_CONFIG: Record<CheckStatus, { color: string; label: string }> = {
  pass: { color: '#22c55e', label: 'Pass' },
  fail: { color: '#ef4444', label: 'Fail' },
  warn: { color: '#f59e0b', label: 'Warning' },
  error: { color: '#6b7280', label: 'Error' },
};

// Added: Icon component for check status
function StatusIcon({ status }: { status: CheckStatus }) {
  const size = 18;
  switch (status) {
    case 'pass':
      return <CheckCircle size={size} style={{ color: STATUS_CONFIG.pass.color }} />;
    case 'fail':
      return <XCircle size={size} style={{ color: STATUS_CONFIG.fail.color }} />;
    case 'warn':
      return <AlertTriangle size={size} style={{ color: STATUS_CONFIG.warn.color }} />;
    case 'error':
      return <AlertOctagon size={size} style={{ color: STATUS_CONFIG.error.color }} />;
  }
}

// Added: Score color based on value
function getScoreColor(score: number): string {
  if (score >= 80) return '#22c55e';
  if (score >= 60) return '#f59e0b';
  return '#ef4444';
}

// Added: TMAIL-39 — External tools sub-panel. Kept in this file because it shares the
// component's domain state and only exists in service of the deliverability flow, but
// rendered separately so the score section above stays unchanged.
function ExternalToolsPanel({
  data,
  isLoading,
  isError,
  onGenerate,
  domain,
}: {
  data: ExternalToolsResponse | undefined;
  isLoading: boolean;
  isError: boolean;
  onGenerate: () => void;
  domain: string;
}) {
  // Added: copy-to-clipboard with a transient "Copied!" badge so the user gets feedback
  // when the mail-tester address lands on the clipboard.
  const [copied, setCopied] = useState(false);
  const copyAddress = async (addr: string) => {
    try {
      await navigator.clipboard.writeText(addr);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard API blocked (e.g. insecure context). The address is still visible
      // and selectable in the read-only input, so silently no-op rather than alert.
    }
  };

  return (
    <div
      style={{
        marginTop: '32px',
        padding: '20px',
        background: 'var(--color-bg-elevated, #f9fafb)',
        borderRadius: '12px',
        border: '1px solid var(--color-border, #e5e7eb)',
      }}
      data-testid="external-tools-panel"
    >
      <h3 style={{ margin: '0 0 8px', fontSize: '16px' }}>
        External Deliverability Tests
      </h3>
      <p
        style={{
          color: 'var(--color-text-secondary)',
          fontSize: '13px',
          margin: '0 0 16px',
        }}
      >
        DNS scans above confirm your config is plausible; these tools verify inbox
        placement at real mailbox providers.
      </p>

      <button
        type="button"
        className="btn btn--secondary"
        onClick={onGenerate}
        disabled={!domain.trim() || isLoading}
        data-testid="generate-external-tools-btn"
        style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
      >
        {isLoading ? (
          <>
            <RefreshCw size={16} className="spin" /> Loading...
          </>
        ) : (
          <>
            <Mail size={16} /> Generate Mail-Tester Address & Postmaster Link
          </>
        )}
      </button>

      {isError && (
        <div
          style={{
            marginTop: '12px',
            padding: '12px',
            background: '#fef2f2',
            border: '1px solid #fecaca',
            borderRadius: '8px',
            color: '#dc2626',
            fontSize: '13px',
          }}
          data-testid="external-tools-error"
        >
          Could not load external deliverability tools. Try again in a moment.
        </div>
      )}

      {data && (
        <div style={{ marginTop: '20px', display: 'flex', flexDirection: 'column', gap: '20px' }}>
          {/* mail-tester.com section */}
          <section data-testid="mail-tester-section">
            <h4 style={{ margin: '0 0 6px', fontSize: '14px' }}>
              mail-tester.com (spam score 0–10)
            </h4>
            <p
              style={{
                margin: '0 0 8px',
                fontSize: '13px',
                color: 'var(--color-text-secondary)',
              }}
            >
              {data.mail_tester.instructions} Target score: 8/10 or higher.
            </p>
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center', marginBottom: '8px' }}>
              <input
                type="text"
                className="input"
                readOnly
                value={data.mail_tester.test_address}
                style={{ flex: 1, fontFamily: 'monospace', fontSize: '13px' }}
                data-testid="mail-tester-address"
              />
              <button
                type="button"
                className="btn btn--ghost"
                onClick={() => copyAddress(data.mail_tester.test_address)}
                data-testid="mail-tester-copy-btn"
                style={{ display: 'flex', alignItems: 'center', gap: '4px' }}
              >
                <Copy size={14} /> {copied ? 'Copied!' : 'Copy'}
              </button>
            </div>
            <a
              href={data.mail_tester.report_url}
              target="_blank"
              rel="noopener noreferrer"
              data-testid="mail-tester-report-link"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                fontSize: '13px',
              }}
            >
              View report (expires in ~{data.mail_tester.expires_in_minutes} min){' '}
              <ExternalLink size={12} />
            </a>
          </section>

          {/* Google Postmaster Tools section */}
          <section data-testid="postmaster-section">
            <h4 style={{ margin: '0 0 6px', fontSize: '14px' }}>
              Google Postmaster Tools (Gmail reputation)
            </h4>
            <p
              style={{
                margin: '0 0 8px',
                fontSize: '13px',
                color: 'var(--color-text-secondary)',
              }}
            >
              {data.google_postmaster.instructions}
            </p>
            <a
              href={data.google_postmaster.dashboard_url}
              target="_blank"
              rel="noopener noreferrer"
              data-testid="postmaster-link"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '4px',
                fontSize: '13px',
              }}
            >
              Open Postmaster Tools <ExternalLink size={12} />
            </a>
          </section>

          {/* Manual provider checklist */}
          <section data-testid="provider-checklist">
            <h4 style={{ margin: '0 0 6px', fontSize: '14px' }}>
              Manual Inbox Placement Checklist
            </h4>
            <p
              style={{
                margin: '0 0 8px',
                fontSize: '13px',
                color: 'var(--color-text-secondary)',
              }}
            >
              Send a test message to a real account at each provider and confirm it
              lands in Inbox — not the spam folder.
            </p>
            <ul style={{ margin: 0, paddingLeft: '20px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {data.providers.map((p) => (
                <li
                  key={p.name}
                  data-testid={`provider-${p.name.toLowerCase().replace(/[^a-z]/g, '-')}`}
                  style={{ fontSize: '13px' }}
                >
                  <strong>{p.name}</strong>{' '}
                  <span style={{ color: 'var(--color-text-secondary)' }}>
                    — spam folder: <code>{p.spam_folder_label}</code>
                  </span>
                  <div
                    style={{
                      color: 'var(--color-text-secondary)',
                      marginTop: '2px',
                    }}
                  >
                    {p.instructions}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        </div>
      )}
    </div>
  );
}

export function DeliverabilityReport() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [domain, setDomain] = useState('');
  const [expandedChecks, setExpandedChecks] = useState<Set<number>>(new Set());

  // Added: Mutation for running the check (not a query since it's on-demand)
  const checkMutation = useMutation({
    mutationFn: (d: string) => runDeliverabilityCheck(d),
  });

  // Added: TMAIL-39 — mutation for the external tools panel; each fire mints a fresh
  // mail-tester handle, so a mutation (action) is the right shape rather than a query.
  const externalToolsMutation = useMutation({
    mutationFn: (d: string) => getExternalDeliverabilityTools(d),
  });

  // Added: Toggle expanded state for a check's details
  const toggleExpanded = (index: number) => {
    setExpandedChecks((prev) => {
      const next = new Set(prev);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      return next;
    });
  };

  // Added: Handle form submission
  const handleRunCheck = (e: React.FormEvent) => {
    e.preventDefault();
    if (domain.trim()) {
      setExpandedChecks(new Set());
      checkMutation.mutate(domain.trim());
    }
  };

  const report = checkMutation.data;

  return (
    <div style={{ padding: '24px', maxWidth: '800px' }}>
      {/* Added: Back navigation */}
      <button
        className="btn btn--ghost"
        onClick={() => setViewMode('list')}
        style={{ marginBottom: '16px', display: 'flex', alignItems: 'center', gap: '6px' }}
      >
        <ArrowLeft size={16} /> Back
      </button>

      <h2 style={{ margin: '0 0 8px' }}>Email Deliverability Check</h2>
      <p style={{ color: 'var(--color-text-secondary)', margin: '0 0 24px' }}>
        Test your mail server's deliverability by checking DNS records, blacklists, TLS, and connectivity.
      </p>

      {/* Added: Domain input form */}
      <form onSubmit={handleRunCheck} style={{ display: 'flex', gap: '8px', marginBottom: '24px' }}>
        <input
          type="text"
          className="input"
          placeholder="mail.example.com"
          value={domain}
          onChange={(e) => setDomain(e.target.value)}
          style={{ flex: 1 }}
          data-testid="domain-input"
        />
        <button
          type="submit"
          className="btn btn--primary"
          disabled={!domain.trim() || checkMutation.isPending}
          data-testid="run-check-btn"
          style={{ display: 'flex', alignItems: 'center', gap: '6px' }}
        >
          {checkMutation.isPending ? (
            <>
              <RefreshCw size={16} className="spin" /> Running...
            </>
          ) : (
            <>
              <Play size={16} /> Run Check
            </>
          )}
        </button>
      </form>

      {/* Added: Error display */}
      {checkMutation.isError && (
        <div
          style={{
            padding: '12px',
            background: '#fef2f2',
            border: '1px solid #fecaca',
            borderRadius: '8px',
            color: '#dc2626',
            marginBottom: '16px',
          }}
          data-testid="error-message"
        >
          Failed to run deliverability check: {checkMutation.error?.message || 'Unknown error'}
        </div>
      )}

      {/* Added: Score display */}
      {report && (
        <div data-testid="report-results">
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '16px',
              marginBottom: '24px',
              padding: '20px',
              background: 'var(--color-bg-elevated, #f9fafb)',
              borderRadius: '12px',
              border: '1px solid var(--color-border, #e5e7eb)',
            }}
          >
            <div
              style={{
                width: '80px',
                height: '80px',
                borderRadius: '50%',
                border: `4px solid ${getScoreColor(report.score)}`,
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontSize: '28px',
                fontWeight: 'bold',
                color: getScoreColor(report.score),
              }}
              data-testid="score-display"
            >
              {report.score}
            </div>
            <div>
              <div style={{ fontSize: '18px', fontWeight: '600' }}>
                Deliverability Score for {report.domain}
              </div>
              <div style={{ color: 'var(--color-text-secondary)', marginTop: '4px' }}>
                {report.checks.filter((c) => c.status === 'pass').length} of {report.checks.length} checks passed
              </div>
            </div>
          </div>

          {/* Added: Check results list */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
            {report.checks.map((check: CheckResult, index: number) => (
              <div
                key={index}
                style={{
                  border: '1px solid var(--color-border, #e5e7eb)',
                  borderRadius: '8px',
                  overflow: 'hidden',
                }}
              >
                <button
                  onClick={() => toggleExpanded(index)}
                  style={{
                    width: '100%',
                    padding: '12px 16px',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    textAlign: 'left',
                    fontSize: '14px',
                  }}
                  data-testid={`check-item-${index}`}
                >
                  <StatusIcon status={check.status} />
                  <span style={{ flex: 1, fontWeight: '500' }}>{check.name}</span>
                  <span
                    style={{
                      padding: '2px 8px',
                      borderRadius: '4px',
                      fontSize: '12px',
                      fontWeight: '600',
                      color: STATUS_CONFIG[check.status].color,
                      background: `${STATUS_CONFIG[check.status].color}15`,
                    }}
                  >
                    {STATUS_CONFIG[check.status].label}
                  </span>
                </button>
                {expandedChecks.has(index) && (
                  <div
                    style={{
                      padding: '8px 16px 12px 46px',
                      color: 'var(--color-text-secondary)',
                      fontSize: '13px',
                      borderTop: '1px solid var(--color-border, #e5e7eb)',
                    }}
                    data-testid={`check-details-${index}`}
                  >
                    {check.details}
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* Added: TMAIL-39 — external deliverability tools (mail-tester + Postmaster
              Tools + manual provider checklist). Rendered after the DNS scorecard so
              admins see "here's your config status" then "here's how to test inbox
              placement" without leaving the page. */}
          <ExternalToolsPanel
            data={externalToolsMutation.data}
            isLoading={externalToolsMutation.isPending}
            isError={externalToolsMutation.isError}
            onGenerate={() => externalToolsMutation.mutate(report.domain)}
            domain={report.domain}
          />
        </div>
      )}
    </div>
  );
}
