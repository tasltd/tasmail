// TMAIL-354: Modern UI Admin → eDiscovery sub-tab. Lists compliance
// searches and lets investigators create new ones, execute pending
// searches, view results, and export completed searches to mbox/eml/pdf.
//
// Backed by /api/admin/ediscovery (CRUD + execute + export). Authorization
// admits is_admin OR is_compliance_officer (require_compliance on the
// backend) — the UI shows the "Admin only" block for non-admins
// regardless because the AdminDashboard shell already gates that.
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Play, Download, Eye, AlertCircle, Search, X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import {
  adminEdiscoveryApi,
  type EdiscoverySearch,
  type EdiscoveryStatus,
  type ExportFormat,
  type CreateEdiscoveryRequest,
  type EdiscoverySearchWithResults,
} from '@/api/admin-ediscovery';

const EXPORT_FORMATS: ExportFormat[] = ['mbox', 'eml', 'pdf'];

const EMPTY_CREATE: CreateEdiscoveryRequest = {
  name: '',
  description: '',
  search_query: '',
  include_attachments: false,
  legal_hold_only: false,
  export_format: 'mbox',
};

function statusVariant(s: EdiscoveryStatus): 'default' | 'secondary' | 'destructive' {
  if (s === 'failed') return 'destructive';
  if (s === 'completed' || s === 'exported') return 'default';
  return 'secondary';
}

export function EdiscoveryTab() {
  const qc = useQueryClient();
  const listQ = useQuery<EdiscoverySearch[]>({
    queryKey: ['admin', 'ediscovery'],
    queryFn: () => adminEdiscoveryApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [viewing, setViewing] = useState<string | null>(null);
  const [form, setForm] = useState<CreateEdiscoveryRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateEdiscoveryRequest) => adminEdiscoveryApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'ediscovery'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const executeMut = useMutation({
    mutationFn: (id: string) => adminEdiscoveryApi.execute(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'ediscovery'] }),
    onError: (e: Error) => setError(e.message),
  });
  const exportMut = useMutation({
    mutationFn: ({ id, format }: { id: string; format?: ExportFormat }) =>
      adminEdiscoveryApi.export(id, format),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'ediscovery'] }),
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminEdiscoveryApi.delete(id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'ediscovery'] });
      setViewing(null);
    },
    onError: (e: Error) => setError(e.message),
  });

  const submit = () => {
    setError(null);
    if (!form.name.trim()) {
      setError('Search name is required.');
      return;
    }
    if (!form.search_query.trim()) {
      setError('Search query is required.');
      return;
    }
    createMut.mutate({
      ...form,
      name: form.name.trim(),
      search_query: form.search_query.trim(),
      description: form.description?.trim() || undefined,
    });
  };

  return (
    <div className="space-y-4" data-testid="ediscovery-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <Search className="size-5" /> eDiscovery
          </h2>
          <p className="text-sm text-zinc-500">
            Create compliance searches across mailboxes, execute them, and
            export results in mbox / eml / pdf. Searches scoped to active
            legal-hold mailboxes when "Legal-hold only" is set.
          </p>
        </div>
        <Button
          onClick={() => { closeForm(); setShowForm(true); }}
          data-testid="ediscovery-add-button"
        >
          <Plus className="size-4 mr-2" /> New search
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
        <Card className="p-6 space-y-4" data-testid="ediscovery-form">
          <h3 className="text-lg font-medium">New eDiscovery search</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Search name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="Case #2026-001 — Acme contract"
                data-testid="ediscovery-form-name"
              />
            </Field>
            <Field label="Export format">
              <select
                className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm w-full"
                value={form.export_format ?? 'mbox'}
                onChange={(e) =>
                  setForm((p) => ({ ...p, export_format: e.target.value as ExportFormat }))
                }
              >
                {EXPORT_FORMATS.map((f) => <option key={f} value={f}>{f}</option>)}
              </select>
            </Field>
          </div>
          <Field label="Search query" required>
            <Input
              value={form.search_query}
              onChange={(e) => setForm((p) => ({ ...p, search_query: e.target.value }))}
              placeholder='from:acme.com OR subject:"NDA"'
              data-testid="ediscovery-form-query"
            />
          </Field>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Date from (optional)">
              <Input
                type="datetime-local"
                value={form.date_from ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, date_from: e.target.value || undefined }))}
              />
            </Field>
            <Field label="Date to (optional)">
              <Input
                type="datetime-local"
                value={form.date_to ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, date_to: e.target.value || undefined }))}
              />
            </Field>
          </div>
          <div className="flex gap-6 text-sm">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={!!form.include_attachments}
                onChange={(e) =>
                  setForm((p) => ({ ...p, include_attachments: e.target.checked }))
                }
              />
              Include attachments
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={!!form.legal_hold_only}
                onChange={(e) =>
                  setForm((p) => ({ ...p, legal_hold_only: e.target.checked }))
                }
                data-testid="ediscovery-form-legal-hold-only"
              />
              Scope to active legal-hold mailboxes
            </label>
          </div>
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
              disabled={createMut.isPending}
              data-testid="ediscovery-form-submit"
            >
              Create search
            </Button>
          </div>
        </Card>
      )}

      {viewing && (
        <EdiscoveryDetailPane
          id={viewing}
          onClose={() => setViewing(null)}
        />
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading searches…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">
            Couldn't load searches: {String(listQ.error)}
          </div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="ediscovery-empty">
            No eDiscovery searches yet. Click "New search" to create one.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((s) => (
              <li
                key={s.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="ediscovery-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-medium">{s.name}</span>
                    <Badge variant={statusVariant(s.status)}>{s.status}</Badge>
                    <Badge variant="outline">{s.export_format}</Badge>
                    {s.legal_hold_only && (
                      <Badge variant="secondary">legal-hold only</Badge>
                    )}
                    {s.results_count !== null && (
                      <span className="text-xs text-zinc-500">
                        {s.results_count} results
                      </span>
                    )}
                  </div>
                  <div className="text-xs text-zinc-500 font-mono break-all">
                    {s.search_query}
                  </div>
                  {s.description && (
                    <div className="text-xs text-zinc-500">{s.description}</div>
                  )}
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setViewing(s.id)}
                    title="View results"
                    data-testid="ediscovery-view-button"
                  >
                    <Eye className="size-4" />
                  </Button>
                  {s.status === 'pending' && (
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={executeMut.isPending}
                      onClick={() => executeMut.mutate(s.id)}
                      title="Execute search"
                      data-testid="ediscovery-execute-button"
                    >
                      <Play className="size-4" />
                    </Button>
                  )}
                  {s.status === 'completed' && (
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={exportMut.isPending}
                      onClick={() => exportMut.mutate({ id: s.id })}
                      title="Export results"
                      data-testid="ediscovery-export-button"
                    >
                      <Download className="size-4" />
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
                    disabled={deleteMut.isPending}
                    onClick={() => {
                      if (window.confirm(`Delete eDiscovery search "${s.name}"?`)) {
                        deleteMut.mutate(s.id);
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

function EdiscoveryDetailPane({ id, onClose }: { id: string; onClose: () => void }) {
  const q = useQuery<EdiscoverySearchWithResults>({
    queryKey: ['admin', 'ediscovery', id],
    queryFn: () => adminEdiscoveryApi.get(id),
  });

  return (
    <Card className="p-6 space-y-3" data-testid="ediscovery-detail">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-medium">Search results</h3>
        <Button variant="ghost" size="sm" onClick={onClose} title="Close">
          <X className="size-4" />
        </Button>
      </div>
      {q.isLoading ? (
        <div className="text-sm text-zinc-500">Loading results…</div>
      ) : q.isError ? (
        <div className="text-sm text-red-600">
          Couldn't load search: {String(q.error)}
        </div>
      ) : !q.data ? null : q.data.results.length === 0 ? (
        <div className="text-sm text-zinc-500" data-testid="ediscovery-results-empty">
          No results yet. Status: {q.data.status}.
        </div>
      ) : (
        <ul className="space-y-2">
          {q.data.results.map((r) => (
            <li
              key={r.id}
              className="p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg text-sm"
              data-testid="ediscovery-result-row"
            >
              <div className="flex items-center gap-2">
                <span className="font-medium">{r.subject ?? '(no subject)'}</span>
                <span className="text-xs text-zinc-500">
                  {r.from_address ?? '(unknown sender)'}
                </span>
              </div>
              {r.snippet && (
                <div className="text-xs text-zinc-500 mt-1">{r.snippet}</div>
              )}
              <div className="text-xs text-zinc-400 font-mono">
                {r.folder} · uid {r.uid}
              </div>
            </li>
          ))}
        </ul>
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
