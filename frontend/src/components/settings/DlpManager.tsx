// Added: DLP management UI for Data Loss Prevention rules and violations (TMAIL-108)
// PURPOSE: Allows admins to create/manage DLP rules, view violations, and test scan text
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ToggleLeft, ToggleRight, ShieldCheck, AlertTriangle, Search } from 'lucide-react';
import {
  listDlpRules,
  createDlpRule,
  updateDlpRule,
  deleteDlpRule,
  listDlpViolations,
  testDlpScan,
} from '../../api/dlp';
import type {
  DlpRule,
  DlpViolation,
  DlpAction,
  DlpSeverity,
  DlpPatternType,
  DlpScanMatch,
} from '../../api/dlp';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: Severity badge color mapping for visual distinction
const SEVERITY_COLORS: Record<DlpSeverity, string> = {
  low: '#6b7280',
  medium: '#f59e0b',
  high: '#f97316',
  critical: '#ef4444',
};

// NOTE: Action badge color mapping
const ACTION_COLORS: Record<DlpAction, string> = {
  block: '#ef4444',
  quarantine: '#f97316',
  warn: '#f59e0b',
  log: '#6b7280',
};

// Added: Severity badge sub-component for consistent rendering
function SeverityBadge({ severity }: { severity: DlpSeverity }) {
  return (
    <span
      style={{
        fontSize: '11px',
        padding: '1px 6px',
        borderRadius: '10px',
        background: SEVERITY_COLORS[severity],
        color: 'white',
        textTransform: 'uppercase',
      }}
      data-testid={`severity-${severity}`}
    >
      {severity}
    </span>
  );
}

// Added: Action badge sub-component
function ActionBadge({ action }: { action: DlpAction }) {
  return (
    <span
      style={{
        fontSize: '11px',
        padding: '1px 6px',
        borderRadius: '10px',
        background: ACTION_COLORS[action],
        color: 'white',
        textTransform: 'uppercase',
      }}
    >
      {action}
    </span>
  );
}

// Added: Violations log tab content
function ViolationsLog() {
  const { data: violations, isLoading } = useQuery({
    queryKey: ['dlp-violations'],
    queryFn: () => listDlpViolations(),
  });

  if (isLoading) return <LoadingSkeleton rows={5} />;

  if (!violations || violations.length === 0) {
    return (
      <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
        No DLP violations recorded yet.
      </p>
    );
  }

  return (
    <div data-testid="violations-log">
      <table style={{ width: '100%', fontSize: '13px', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '2px solid var(--color-border)' }}>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Date</th>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Action</th>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Pattern</th>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Match</th>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Subject</th>
            <th style={{ textAlign: 'left', padding: '6px 8px' }}>Recipient</th>
          </tr>
        </thead>
        <tbody>
          {violations.map((violation: DlpViolation) => (
            <tr key={violation.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
              <td style={{ padding: '6px 8px' }}>
                {new Date(violation.created_at).toLocaleString()}
              </td>
              <td style={{ padding: '6px 8px' }}>
                <ActionBadge action={violation.action_taken} />
              </td>
              <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontSize: '12px' }}>
                {violation.matched_pattern.length > 30
                  ? violation.matched_pattern.substring(0, 30) + '...'
                  : violation.matched_pattern}
              </td>
              <td style={{ padding: '6px 8px' }}>{violation.matched_text ?? '—'}</td>
              <td style={{ padding: '6px 8px' }}>{violation.message_subject ?? '—'}</td>
              <td style={{ padding: '6px 8px' }}>{violation.recipient ?? '—'}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

// Added: Test scan panel for dry-run testing of DLP rules
function TestScanPanel() {
  const [scanSubject, setScanSubject] = useState('');
  const [scanBody, setScanBody] = useState('');
  const [scanResults, setScanResults] = useState<DlpScanMatch[] | null>(null);

  const scanMut = useMutation({
    mutationFn: testDlpScan,
    onSuccess: (matches) => setScanResults(matches),
  });

  const handleScan = (e: FormEvent) => {
    e.preventDefault();
    scanMut.mutate({ subject: scanSubject || undefined, body: scanBody || undefined });
  };

  return (
    <div
      style={{
        marginTop: '16px',
        padding: '16px',
        border: '1px solid var(--color-border)',
        borderRadius: '8px',
      }}
      data-testid="test-scan-panel"
    >
      <h3 style={{ marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
        <Search size={18} /> Test Scan
      </h3>
      <form onSubmit={handleScan}>
        <div className="composer__field">
          <label>Subject</label>
          <input
            value={scanSubject}
            onChange={(e) => setScanSubject(e.target.value)}
            placeholder="Optional subject text to scan"
          />
        </div>
        <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
          <label>Body</label>
          <textarea
            value={scanBody}
            onChange={(e) => setScanBody(e.target.value)}
            placeholder="Paste email body text to test against DLP rules..."
            rows={4}
            style={{ width: '100%', resize: 'vertical' }}
          />
        </div>
        <div className="composer__actions">
          <button type="submit" className="btn btn--primary" disabled={scanMut.isPending}>
            {scanMut.isPending ? 'Scanning...' : 'Run Scan'}
          </button>
        </div>
      </form>

      {/* Added: Display scan results */}
      {scanResults !== null && (
        <div style={{ marginTop: '12px' }}>
          {scanResults.length === 0 ? (
            <p style={{ color: 'green', fontWeight: 600 }}>No DLP violations found.</p>
          ) : (
            <div>
              <p style={{ color: '#ef4444', fontWeight: 600, marginBottom: '8px' }}>
                {scanResults.length} violation{scanResults.length !== 1 ? 's' : ''} detected:
              </p>
              {scanResults.map((match, index) => (
                <div
                  key={index}
                  style={{
                    padding: '8px',
                    marginBottom: '4px',
                    background: 'var(--color-bg-secondary, #f9fafb)',
                    borderRadius: '4px',
                    fontSize: '13px',
                  }}
                >
                  <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                    <AlertTriangle size={14} style={{ color: SEVERITY_COLORS[match.severity] }} />
                    <strong>{match.rule_name}</strong>
                    <SeverityBadge severity={match.severity} />
                    <ActionBadge action={match.action} />
                  </div>
                  <div style={{ marginTop: '4px', fontFamily: 'monospace', fontSize: '12px' }}>
                    Matched: &quot;{match.matched_text}&quot;
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function DlpManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [activeTab, setActiveTab] = useState<'rules' | 'violations' | 'scan'>('rules');

  // Added: Form state for creating new DLP rules
  const [formName, setFormName] = useState('');
  const [formPattern, setFormPattern] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formPatternType, setFormPatternType] = useState<DlpPatternType>('regex');
  const [formAction, setFormAction] = useState<DlpAction>('warn');
  const [formSeverity, setFormSeverity] = useState<DlpSeverity>('medium');
  const [formApplySubject, setFormApplySubject] = useState(true);
  const [formApplyBody, setFormApplyBody] = useState(true);

  const { data: rules, isLoading } = useQuery({
    queryKey: ['dlp-rules'],
    queryFn: listDlpRules,
  });

  const createMut = useMutation({
    mutationFn: createDlpRule,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dlp-rules'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      setFormName('');
      setFormPattern('');
      setFormDescription('');
      setFormPatternType('regex');
      setFormAction('warn');
      setFormSeverity('medium');
      setFormApplySubject(true);
      setFormApplyBody(true);
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      updateDlpRule(id, { active }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['dlp-rules'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteDlpRule,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['dlp-rules'] }),
  });

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      name: formName,
      pattern: formPattern,
      description: formDescription || undefined,
      pattern_type: formPatternType,
      action: formAction,
      severity: formSeverity,
      apply_to_subject: formApplySubject,
      apply_to_body: formApplyBody,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="dlp-manager" style={{ padding: '16px', maxWidth: '1000px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Data Loss Prevention</h2>
        {activeTab === 'rules' && (
          <button className="btn btn--primary" onClick={() => setIsCreating(true)}>
            <Plus size={16} /> Add Rule
          </button>
        )}
      </div>

      {/* Added: Tab navigation for rules, violations, and test scan */}
      <div style={{ display: 'flex', gap: '4px', marginTop: '12px', borderBottom: '1px solid var(--color-border)' }}>
        {(['rules', 'violations', 'scan'] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              padding: '8px 16px',
              background: activeTab === tab ? 'var(--color-primary, #3b82f6)' : 'transparent',
              color: activeTab === tab ? 'white' : 'inherit',
              border: 'none',
              borderRadius: '4px 4px 0 0',
              cursor: 'pointer',
              textTransform: 'capitalize',
            }}
          >
            {tab === 'scan' ? 'Test Scan' : tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Added: Rules tab content */}
      {activeTab === 'rules' && (
        <>
          {/* Added: Create DLP rule form */}
          {isCreating && (
            <div
              style={{
                marginTop: '16px',
                padding: '16px',
                border: '1px solid var(--color-border)',
                borderRadius: '8px',
              }}
            >
              <h3 style={{ marginBottom: '12px' }}>New DLP Rule</h3>
              <form onSubmit={handleCreate}>
                <div className="composer__field">
                  <label>Name</label>
                  <input
                    value={formName}
                    onChange={(e) => setFormName(e.target.value)}
                    placeholder="Credit Card Blocker"
                    required
                  />
                </div>
                <div className="composer__field">
                  <label>Pattern</label>
                  <input
                    value={formPattern}
                    onChange={(e) => setFormPattern(e.target.value)}
                    placeholder="\b\d{4}[- ]?\d{4}[- ]?\d{4}[- ]?\d{4}\b"
                    required
                  />
                </div>
                <div className="composer__field">
                  <label>Description</label>
                  <input
                    value={formDescription}
                    onChange={(e) => setFormDescription(e.target.value)}
                    placeholder="Optional description"
                  />
                </div>
                <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '12px' }}>
                  <div className="composer__field" style={{ flex: '1 1 150px' }}>
                    <label>Pattern Type</label>
                    <select value={formPatternType} onChange={(e) => setFormPatternType(e.target.value as DlpPatternType)}>
                      <option value="regex">Regex</option>
                      <option value="keyword">Keyword</option>
                      <option value="dictionary">Dictionary</option>
                    </select>
                  </div>
                  <div className="composer__field" style={{ flex: '1 1 150px' }}>
                    <label>Action</label>
                    <select value={formAction} onChange={(e) => setFormAction(e.target.value as DlpAction)}>
                      <option value="block">Block</option>
                      <option value="quarantine">Quarantine</option>
                      <option value="warn">Warn</option>
                      <option value="log">Log</option>
                    </select>
                  </div>
                  <div className="composer__field" style={{ flex: '1 1 150px' }}>
                    <label>Severity</label>
                    <select value={formSeverity} onChange={(e) => setFormSeverity(e.target.value as DlpSeverity)}>
                      <option value="low">Low</option>
                      <option value="medium">Medium</option>
                      <option value="high">High</option>
                      <option value="critical">Critical</option>
                    </select>
                  </div>
                </div>
                <div style={{ display: 'flex', gap: '16px', marginBottom: '12px' }}>
                  <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={formApplySubject}
                      onChange={(e) => setFormApplySubject(e.target.checked)}
                    />
                    Apply to subject
                  </label>
                  <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}>
                    <input
                      type="checkbox"
                      checked={formApplyBody}
                      onChange={(e) => setFormApplyBody(e.target.checked)}
                    />
                    Apply to body
                  </label>
                </div>
                <div className="composer__actions">
                  <button type="submit" className="btn btn--primary" disabled={!formName || !formPattern}>
                    Create
                  </button>
                  <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                    Cancel
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* Added: Rules list */}
          <div style={{ marginTop: '16px' }}>
            {(!rules || rules.length === 0) && !isCreating && (
              <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
                No DLP rules configured. Add one to scan outgoing emails for sensitive data.
              </p>
            )}
            {rules?.map((rule: DlpRule) => (
              <div
                key={rule.id}
                style={{
                  padding: '12px',
                  borderBottom: '1px solid var(--color-border)',
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <ShieldCheck size={18} style={{ color: 'var(--color-text-secondary)' }} />
                  <div style={{ flex: 1 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <strong style={{ fontSize: '14px' }}>{rule.name}</strong>
                      <SeverityBadge severity={rule.severity} />
                      <ActionBadge action={rule.action} />
                      <span
                        style={{
                          fontSize: '11px',
                          padding: '1px 6px',
                          borderRadius: '10px',
                          background: rule.active ? 'green' : 'gray',
                          color: 'white',
                        }}
                      >
                        {rule.active ? 'Active' : 'Inactive'}
                      </span>
                    </div>
                    <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                      {rule.pattern_type} &middot;{' '}
                      <code style={{ fontSize: '11px' }}>
                        {rule.pattern.length > 50 ? rule.pattern.substring(0, 50) + '...' : rule.pattern}
                      </code>
                      {rule.description && <> &middot; {rule.description}</>}
                    </div>
                  </div>
                  {/* Added: Active/inactive toggle */}
                  <button
                    className="btn btn--icon"
                    onClick={() => toggleMut.mutate({ id: rule.id, active: !rule.active })}
                    title={rule.active ? 'Deactivate' : 'Activate'}
                    data-testid={`toggle-${rule.id}`}
                  >
                    {rule.active ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
                  </button>
                  {/* Added: Delete button */}
                  <button
                    className="btn btn--icon btn--danger"
                    onClick={() => deleteMut.mutate(rule.id)}
                    title="Delete"
                  >
                    <Trash2 size={16} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        </>
      )}

      {/* Added: Violations tab content */}
      {activeTab === 'violations' && <ViolationsLog />}

      {/* Added: Test scan tab content */}
      {activeTab === 'scan' && <TestScanPanel />}
    </div>
  );
}
