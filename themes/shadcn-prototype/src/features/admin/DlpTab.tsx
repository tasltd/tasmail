// TMAIL-354: Modern UI Admin → DLP sub-tab. Three nested panes:
//   1. Rules — CRUD against /api/admin/dlp/rules. Regex / keyword / dict
//      patterns with action (block|quarantine|warn|log) + severity.
//   2. Violations — paginated log of matches, /api/admin/dlp/violations.
//   3. Test scan — dry-run a subject/body against active rules via
//      POST /api/admin/dlp/scan so admins can verify a pattern before
//      switching its action to "block".
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Edit2, AlertCircle, Shield, FlaskConical, AlertTriangle,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import {
  adminDlpApi,
  type DlpRule,
  type DlpViolation,
  type DlpAction,
  type DlpSeverity,
  type CreateDlpRuleRequest,
  type UpdateDlpRuleRequest,
  type DlpScanMatch,
} from '@/api/admin-dlp';

const ACTIONS: DlpAction[] = ['log', 'warn', 'quarantine', 'block'];
const SEVERITIES: DlpSeverity[] = ['low', 'medium', 'high', 'critical'];
const PATTERN_TYPES = ['regex', 'keyword', 'dictionary'] as const;

const EMPTY_RULE: CreateDlpRuleRequest = {
  name: '',
  description: '',
  pattern: '',
  pattern_type: 'regex',
  action: 'log',
  severity: 'medium',
  apply_to_subject: true,
  apply_to_body: true,
  apply_to_attachments: false,
};

function severityVariant(s: DlpSeverity): 'default' | 'secondary' | 'destructive' {
  return s === 'critical' || s === 'high' ? 'destructive' : s === 'medium' ? 'default' : 'secondary';
}

export function DlpTab() {
  return (
    <div className="space-y-4" data-testid="dlp-tab">
      <div>
        <h2 className="text-xl font-semibold flex items-center gap-2">
          <Shield className="size-5" /> Data Loss Prevention (DLP)
        </h2>
        <p className="text-sm text-zinc-500">
          Scan outgoing email for sensitive content patterns
          (credit-card, SSN, IBAN, custom regex) and block, quarantine,
          warn, or log matches.
        </p>
      </div>
      <Tabs defaultValue="rules" className="space-y-4">
        <TabsList>
          <TabsTrigger value="rules" data-testid="dlp-subtab-rules">Rules</TabsTrigger>
          <TabsTrigger value="violations" data-testid="dlp-subtab-violations">Violations</TabsTrigger>
          <TabsTrigger value="scan" data-testid="dlp-subtab-scan">Test scan</TabsTrigger>
        </TabsList>
        <TabsContent value="rules"><DlpRulesPane /></TabsContent>
        <TabsContent value="violations"><DlpViolationsPane /></TabsContent>
        <TabsContent value="scan"><DlpScanPane /></TabsContent>
      </Tabs>
    </div>
  );
}

function DlpRulesPane() {
  const qc = useQueryClient();
  const listQ = useQuery<DlpRule[]>({
    queryKey: ['admin', 'dlp', 'rules'],
    queryFn: () => adminDlpApi.listRules(),
  });

  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<DlpRule | null>(null);
  const [form, setForm] = useState<CreateDlpRuleRequest>(EMPTY_RULE);
  const [error, setError] = useState<string | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(EMPTY_RULE);
    setError(null);
  };

  const startEdit = (r: DlpRule) => {
    setEditing(r);
    setForm({
      name: r.name,
      description: r.description ?? '',
      pattern: r.pattern,
      pattern_type: r.pattern_type,
      action: r.action,
      severity: r.severity,
      apply_to_subject: r.apply_to_subject,
      apply_to_body: r.apply_to_body,
      apply_to_attachments: r.apply_to_attachments,
    });
    setShowForm(true);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateDlpRuleRequest) => adminDlpApi.createRule(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'dlp', 'rules'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateDlpRuleRequest }) =>
      adminDlpApi.updateRule(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'dlp', 'rules'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminDlpApi.deleteRule(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'dlp', 'rules'] }),
    onError: (e: Error) => setError(e.message),
  });

  const submit = () => {
    setError(null);
    if (!form.name.trim() || !form.pattern.trim()) {
      setError('Name and pattern are required.');
      return;
    }
    if (editing) {
      updateMut.mutate({ id: editing.id, body: form });
    } else {
      createMut.mutate(form);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium">DLP rules</h3>
        <Button
          onClick={() => { closeForm(); setShowForm(true); }}
          data-testid="dlp-rule-add-button"
        >
          <Plus className="size-4 mr-2" /> Add rule
        </Button>
      </div>

      {error && (
        <Card className="p-3 border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950">
          <div className="text-sm text-red-700 dark:text-red-300 flex items-center gap-2">
            <AlertCircle className="size-4" /> {error}
          </div>
        </Card>
      )}

      {showForm && (
        <Card className="p-6 space-y-4" data-testid="dlp-rule-form">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Rule name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="Credit-card blocker"
                data-testid="dlp-rule-form-name"
              />
            </Field>
            <Field label="Pattern type">
              <select
                className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm w-full"
                value={form.pattern_type ?? 'regex'}
                onChange={(e) => setForm((p) => ({ ...p, pattern_type: e.target.value }))}
              >
                {PATTERN_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
              </select>
            </Field>
          </div>
          <Field label="Pattern" required>
            <Input
              value={form.pattern}
              onChange={(e) => setForm((p) => ({ ...p, pattern: e.target.value }))}
              placeholder="\\b\\d{4}[- ]?\\d{4}[- ]?\\d{4}[- ]?\\d{4}\\b"
              className="font-mono text-xs"
              data-testid="dlp-rule-form-pattern"
            />
          </Field>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Action">
              <select
                className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm w-full"
                value={form.action ?? 'log'}
                onChange={(e) =>
                  setForm((p) => ({ ...p, action: e.target.value as DlpAction }))
                }
              >
                {ACTIONS.map((a) => <option key={a} value={a}>{a}</option>)}
              </select>
            </Field>
            <Field label="Severity">
              <select
                className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm w-full"
                value={form.severity ?? 'medium'}
                onChange={(e) =>
                  setForm((p) => ({ ...p, severity: e.target.value as DlpSeverity }))
                }
              >
                {SEVERITIES.map((s) => <option key={s} value={s}>{s}</option>)}
              </select>
            </Field>
          </div>
          <Field label="Apply to">
            <div className="flex gap-4 text-sm">
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={!!form.apply_to_subject}
                  onChange={(e) => setForm((p) => ({ ...p, apply_to_subject: e.target.checked }))}
                /> subject
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={!!form.apply_to_body}
                  onChange={(e) => setForm((p) => ({ ...p, apply_to_body: e.target.checked }))}
                /> body
              </label>
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={!!form.apply_to_attachments}
                  onChange={(e) => setForm((p) => ({ ...p, apply_to_attachments: e.target.checked }))}
                /> attachments
              </label>
            </div>
          </Field>
          <Field label="Description">
            <Textarea
              value={form.description ?? ''}
              onChange={(e) => setForm((p) => ({ ...p, description: e.target.value }))}
              rows={2}
            />
          </Field>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={createMut.isPending || updateMut.isPending}
              data-testid="dlp-rule-form-submit"
            >
              {editing ? 'Save changes' : 'Create rule'}
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading rules…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">
            Couldn't load rules: {String(listQ.error)}
          </div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="dlp-rules-empty">
            No DLP rules configured yet.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((r) => (
              <li
                key={r.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="dlp-rule-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-medium">{r.name}</span>
                    <Badge variant="default">{r.action}</Badge>
                    <Badge variant={severityVariant(r.severity)}>{r.severity}</Badge>
                    <Badge variant="outline">{r.pattern_type}</Badge>
                    {!r.active && <Badge variant="secondary">disabled</Badge>}
                  </div>
                  {r.description && (
                    <div className="text-xs text-zinc-500">{r.description}</div>
                  )}
                  <div className="text-xs text-zinc-500 font-mono break-all">
                    {r.pattern}
                  </div>
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button variant="ghost" size="sm" onClick={() => startEdit(r)} title="Edit">
                    <Edit2 className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
                    disabled={deleteMut.isPending}
                    onClick={() => {
                      if (window.confirm(`Delete DLP rule "${r.name}"?`)) {
                        deleteMut.mutate(r.id);
                      }
                    }}
                    title="Delete"
                  >
                    <Trash2 className="size-4" />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function DlpViolationsPane() {
  const [offset, setOffset] = useState(0);
  const limit = 25;
  const violationsQ = useQuery<DlpViolation[]>({
    queryKey: ['admin', 'dlp', 'violations', offset],
    queryFn: () => adminDlpApi.listViolations({ limit, offset }),
  });
  const list = violationsQ.data ?? [];

  return (
    <Card className="p-6" data-testid="dlp-violations-pane">
      {violationsQ.isLoading ? (
        <div className="text-sm text-zinc-500">Loading violations…</div>
      ) : violationsQ.isError ? (
        <div className="text-sm text-red-600">
          Couldn't load violations: {String(violationsQ.error)}
        </div>
      ) : list.length === 0 ? (
        <div className="text-sm text-zinc-500" data-testid="dlp-violations-empty">
          No DLP violations recorded yet.
        </div>
      ) : (
        <ul className="space-y-2">
          {list.map((v) => (
            <li
              key={v.id}
              className="p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg text-sm"
              data-testid="dlp-violation-row"
            >
              <div className="flex items-center gap-2">
                <AlertTriangle className="size-4 text-amber-600" />
                <Badge variant="default">{v.action_taken}</Badge>
                <span className="text-xs text-zinc-500">
                  {new Date(v.created_at).toLocaleString()}
                </span>
              </div>
              {v.message_subject && (
                <div className="mt-1 text-zinc-600 dark:text-zinc-300">
                  Subject: {v.message_subject}
                </div>
              )}
              {v.recipient && (
                <div className="text-xs text-zinc-500">To: {v.recipient}</div>
              )}
              <div className="text-xs font-mono text-zinc-500 break-all">
                Matched: {v.matched_pattern}
              </div>
            </li>
          ))}
        </ul>
      )}
      <div className="flex justify-between items-center mt-4">
        <Button
          variant="outline"
          size="sm"
          disabled={offset === 0}
          onClick={() => setOffset((o) => Math.max(0, o - limit))}
        >
          Previous
        </Button>
        <span className="text-xs text-zinc-500">offset {offset}</span>
        <Button
          variant="outline"
          size="sm"
          disabled={list.length < limit}
          onClick={() => setOffset((o) => o + limit)}
        >
          Next
        </Button>
      </div>
    </Card>
  );
}

function DlpScanPane() {
  const [subject, setSubject] = useState('');
  const [body, setBody] = useState('');
  const [matches, setMatches] = useState<DlpScanMatch[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const scanMut = useMutation({
    mutationFn: () => adminDlpApi.testScan({ subject, body }),
    onSuccess: (data) => { setMatches(data); setError(null); },
    onError: (e: Error) => { setError(e.message); setMatches(null); },
  });

  return (
    <Card className="p-6 space-y-4" data-testid="dlp-scan-pane">
      <p className="text-sm text-zinc-500">
        Paste a subject and body to see which active rules match. No
        message is sent — this is a dry-run against the same scanner the
        outbound queue uses.
      </p>
      <Field label="Subject">
        <Input
          value={subject}
          onChange={(e) => setSubject(e.target.value)}
          placeholder="Q4 invoice"
          data-testid="dlp-scan-subject"
        />
      </Field>
      <Field label="Body">
        <Textarea
          value={body}
          onChange={(e) => setBody(e.target.value)}
          rows={6}
          placeholder="Card on file: 4111 1111 1111 1111"
          data-testid="dlp-scan-body"
        />
      </Field>
      <div className="flex justify-end">
        <Button
          onClick={() => scanMut.mutate()}
          disabled={scanMut.isPending || (!subject && !body)}
          data-testid="dlp-scan-submit"
        >
          <FlaskConical className="size-4 mr-2" />
          {scanMut.isPending ? 'Scanning…' : 'Run scan'}
        </Button>
      </div>
      {error && (
        <div className="text-sm text-red-600 flex items-center gap-2">
          <AlertCircle className="size-4" /> {error}
        </div>
      )}
      {matches !== null && (
        <div data-testid="dlp-scan-results">
          {matches.length === 0 ? (
            <div className="text-sm text-green-600">
              No DLP rules matched — this message would pass.
            </div>
          ) : (
            <ul className="space-y-2">
              {matches.map((m, idx) => (
                <li
                  key={`${m.rule_id}-${idx}`}
                  className="p-3 border border-amber-300 dark:border-amber-800 bg-amber-50 dark:bg-amber-950 rounded-lg text-sm"
                >
                  <div className="flex items-center gap-2">
                    <Badge variant="default">{m.action}</Badge>
                    <Badge variant={severityVariant(m.severity)}>{m.severity}</Badge>
                    <span className="font-medium">{m.rule_name}</span>
                  </div>
                  <div className="text-xs font-mono text-zinc-600 break-all mt-1">
                    Matched: {m.matched_text}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </Card>
  );
}

interface FieldProps {
  label: string;
  required?: boolean;
  children: React.ReactNode;
}

function Field({ label, required, children }: FieldProps) {
  return (
    <div className="space-y-2">
      <Label className="flex items-center gap-1">
        {label}
        {required && <span className="text-red-500">*</span>}
      </Label>
      {children}
    </div>
  );
}
