// Added: Phishing protection management UI for TMAIL-124
// PURPOSE: Allows users to scan messages for phishing, view reports, and update report actions
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, ShieldAlert, ShieldCheck, ShieldOff, Search, AlertTriangle } from 'lucide-react';
import {
  getPhishingReport,
  scanMessage,
  updatePhishingAction,
} from '../../api/phishing';
import type { PhishingReport, UpdateActionRequest } from '../../api/phishing';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Risk level badge colors based on score thresholds
// NOTE: score 0-30 = low, 31-60 = medium, 61-100 = high
function getRiskLevel(score: number): { label: string; bg: string; text: string } {
  if (score <= 30) return { label: 'Low', bg: '#22c55e', text: 'white' };
  if (score <= 60) return { label: 'Medium', bg: '#f59e0b', text: 'white' };
  return { label: 'High', bg: '#ef4444', text: 'white' };
}

// Added: Action badge colors for user action status
const ACTION_COLORS: Record<string, { bg: string; text: string }> = {
  dismissed: { bg: '#6b7280', text: 'white' },
  reported: { bg: '#ef4444', text: 'white' },
  confirmed_safe: { bg: '#22c55e', text: 'white' },
  pending: { bg: '#f59e0b', text: 'white' },
};

export function PhishingManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Scan form state
  const [scanFolder, setScanFolder] = useState('');
  const [scanUid, setScanUid] = useState('');
  const [scanHtmlBody, setScanHtmlBody] = useState('');
  const [scanSenderName, setScanSenderName] = useState('');
  const [scanSenderEmail, setScanSenderEmail] = useState('');

  // Added: Lookup form state for fetching existing report
  const [lookupFolder, setLookupFolder] = useState('');
  const [lookupUid, setLookupUid] = useState('');
  const [currentReport, setCurrentReport] = useState<PhishingReport | null>(null);

  // Added: Fetch phishing report for looked-up message
  const { data: report, isLoading: reportLoading, refetch: refetchReport } = useQuery({
    queryKey: ['phishing-reports', lookupFolder, lookupUid],
    queryFn: () => getPhishingReport(lookupFolder, parseInt(lookupUid, 10)),
    enabled: false,
  });

  // Added: Scan mutation triggers phishing analysis on a message
  const scanMut = useMutation({
    mutationFn: () =>
      scanMessage(scanFolder, parseInt(scanUid, 10), {
        html_body: scanHtmlBody,
        sender_display_name: scanSenderName,
        sender_email: scanSenderEmail,
      }),
    onSuccess: (scanResult) => {
      setCurrentReport(scanResult);
      queryClient.invalidateQueries({ queryKey: ['phishing-reports'] });
    },
  });

  // Added: Update user action on a phishing report
  const actionMut = useMutation({
    mutationFn: ({ reportId, action }: { reportId: string; action: UpdateActionRequest['action'] }) =>
      updatePhishingAction(reportId, { action }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['phishing-reports'] });
      if (lookupFolder && lookupUid) {
        refetchReport();
      }
    },
  });

  const handleScan = (e: React.FormEvent) => {
    e.preventDefault();
    scanMut.mutate();
  };

  const handleLookup = (e: React.FormEvent) => {
    e.preventDefault();
    setCurrentReport(null);
    refetchReport();
  };

  // NOTE: Show report from lookup query or from scan result
  const displayReport = currentReport || report;

  return (
    <div className="phishing-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Phishing Protection</h2>
      </div>

      {/* Added: Scan message form */}
      <div style={{ marginTop: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
        <h3 style={{ marginBottom: '12px' }}>
          <Search size={16} style={{ marginRight: '6px', verticalAlign: 'text-bottom' }} />
          Scan Message
        </h3>
        <form onSubmit={handleScan}>
          <div style={{ display: 'flex', gap: '8px', marginBottom: '8px' }}>
            <div className="composer__field" style={{ flex: 1 }}>
              <label>Folder</label>
              <input
                value={scanFolder}
                onChange={(e) => setScanFolder(e.target.value)}
                placeholder="INBOX"
                required
              />
            </div>
            <div className="composer__field" style={{ width: '120px' }}>
              <label>UID</label>
              <input
                value={scanUid}
                onChange={(e) => setScanUid(e.target.value)}
                placeholder="1"
                type="number"
                required
              />
            </div>
          </div>
          <div className="composer__field" style={{ marginBottom: '8px' }}>
            <label>Sender Display Name</label>
            <input
              value={scanSenderName}
              onChange={(e) => setScanSenderName(e.target.value)}
              placeholder="John Doe"
              required
            />
          </div>
          <div className="composer__field" style={{ marginBottom: '8px' }}>
            <label>Sender Email</label>
            <input
              value={scanSenderEmail}
              onChange={(e) => setScanSenderEmail(e.target.value)}
              placeholder="sender@example.com"
              type="email"
              required
            />
          </div>
          <div className="composer__field" style={{ marginBottom: '8px' }}>
            <label>HTML Body</label>
            <textarea
              value={scanHtmlBody}
              onChange={(e) => setScanHtmlBody(e.target.value)}
              placeholder="<html>...</html>"
              rows={4}
              required
              style={{ width: '100%', resize: 'vertical' }}
            />
          </div>
          <div className="composer__actions">
            <button
              type="submit"
              className="btn btn--primary"
              disabled={!scanFolder.trim() || !scanUid.trim() || scanMut.isPending}
            >
              <ShieldAlert size={16} /> {scanMut.isPending ? 'Scanning...' : 'Scan'}
            </button>
          </div>
        </form>
      </div>

      {/* Added: Lookup existing report form */}
      <div style={{ marginTop: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
        <h3 style={{ marginBottom: '12px' }}>Lookup Report</h3>
        <form onSubmit={handleLookup} style={{ display: 'flex', gap: '8px', alignItems: 'flex-end' }}>
          <div className="composer__field" style={{ flex: 1 }}>
            <label>Folder</label>
            <input
              value={lookupFolder}
              onChange={(e) => setLookupFolder(e.target.value)}
              placeholder="INBOX"
              required
            />
          </div>
          <div className="composer__field" style={{ width: '120px' }}>
            <label>UID</label>
            <input
              value={lookupUid}
              onChange={(e) => setLookupUid(e.target.value)}
              placeholder="1"
              type="number"
              required
            />
          </div>
          <button type="submit" className="btn btn--primary" disabled={!lookupFolder.trim() || !lookupUid.trim()}>
            Lookup
          </button>
        </form>
      </div>

      {/* Added: Loading state for report lookup */}
      {reportLoading && <LoadingSkeleton rows={3} />}

      {/* Added: Display phishing report details */}
      {displayReport && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            background: 'var(--color-bg-secondary)',
          }}
          data-testid="phishing-report"
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px' }}>
            <AlertTriangle size={20} style={{ color: getRiskLevel(displayReport.risk_score).bg }} />
            <h3 style={{ flex: 1, margin: 0 }}>
              Phishing Report — {displayReport.folder} #{displayReport.message_uid}
            </h3>
            {/* Added: Risk level badge */}
            <span
              style={{
                fontSize: '11px',
                padding: '2px 8px',
                borderRadius: '10px',
                background: getRiskLevel(displayReport.risk_score).bg,
                color: getRiskLevel(displayReport.risk_score).text,
                fontWeight: 'bold',
              }}
            >
              {getRiskLevel(displayReport.risk_score).label} Risk ({displayReport.risk_score})
            </span>
          </div>

          <div style={{ fontSize: '13px', marginBottom: '8px' }}>
            <div><strong>Suspicious Sender:</strong> {displayReport.suspicious_sender ? 'Yes' : 'No'}</div>
            <div><strong>Spoofed Display Name:</strong> {displayReport.spoofed_display_name ? 'Yes' : 'No'}</div>
            <div><strong>Scanned:</strong> {new Date(displayReport.created_at).toLocaleString()}</div>
          </div>

          {/* Added: Suspicious links list */}
          {displayReport.suspicious_links.length > 0 && (
            <div style={{ marginTop: '8px' }}>
              <strong style={{ fontSize: '13px' }}>Suspicious Links ({displayReport.suspicious_links.length}):</strong>
              {displayReport.suspicious_links.map((link, index) => (
                <div
                  key={index}
                  style={{
                    marginTop: '4px',
                    padding: '8px',
                    border: '1px solid var(--color-border)',
                    borderRadius: '4px',
                    fontSize: '12px',
                  }}
                >
                  <div><strong>URL:</strong> <code>{link.url}</code></div>
                  <div><strong>Display Text:</strong> {link.display_text}</div>
                  <div><strong>Reasons:</strong> {link.reasons.join(', ')}</div>
                </div>
              ))}
            </div>
          )}

          {/* Added: Action buttons for updating report status */}
          <div style={{ marginTop: '12px', display: 'flex', gap: '8px', alignItems: 'center' }}>
            <span style={{ fontSize: '13px', color: 'var(--color-text-secondary)' }}>
              Current action:
              <span
                style={{
                  marginLeft: '6px',
                  fontSize: '11px',
                  padding: '1px 6px',
                  borderRadius: '10px',
                  background: (ACTION_COLORS[displayReport.user_action] || ACTION_COLORS.pending).bg,
                  color: (ACTION_COLORS[displayReport.user_action] || ACTION_COLORS.pending).text,
                }}
              >
                {displayReport.user_action || 'pending'}
              </span>
            </span>
            <div style={{ flex: 1 }} />
            <button
              className="btn"
              onClick={() => actionMut.mutate({ reportId: displayReport.id, action: 'confirmed_safe' })}
              disabled={actionMut.isPending}
              title="Confirm Safe"
            >
              <ShieldCheck size={14} /> Safe
            </button>
            <button
              className="btn"
              onClick={() => actionMut.mutate({ reportId: displayReport.id, action: 'dismissed' })}
              disabled={actionMut.isPending}
              title="Dismiss"
            >
              <ShieldOff size={14} /> Dismiss
            </button>
            <button
              className="btn btn--danger"
              onClick={() => actionMut.mutate({ reportId: displayReport.id, action: 'reported' })}
              disabled={actionMut.isPending}
              title="Report"
            >
              <ShieldAlert size={14} /> Report
            </button>
          </div>
        </div>
      )}

      {/* Added: Empty state when no report is loaded */}
      {!displayReport && !reportLoading && (
        <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px', marginTop: '16px' }}>
          No phishing report loaded. Use the scan or lookup forms above to check a message.
        </p>
      )}
    </div>
  );
}
