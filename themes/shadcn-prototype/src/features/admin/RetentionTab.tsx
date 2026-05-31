// TMAIL-354: Modern UI Admin → Retention sub-tab. Lists retention policies,
// lets admins create / edit / delete them. Each policy defines how long
// emails are kept (auto-deletion after `retention_days`) and optionally a
// folder pattern to scope to (NULL means all folders).
//
// Backed by /api/admin/retention (CRUD). The list query uses TanStack
// Query so the table refreshes automatically after every mutation.
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Edit2, AlertCircle, Clock,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import {
  adminRetentionApi,
  type RetentionPolicy,
  type CreateRetentionPolicyRequest,
  type UpdateRetentionPolicyRequest,
} from '@/api/admin-retention';

const EMPTY_CREATE: CreateRetentionPolicyRequest = {
  name: '',
  description: '',
  retention_days: 365,
  folder_pattern: '',
  apply_to_all: true,
};

// Drop empty strings so the backend sees `null` rather than the literal
// "" — keeps the optional columns clean.
function toCreateBody(form: CreateRetentionPolicyRequest): CreateRetentionPolicyRequest {
  return {
    name: form.name.trim(),
    description: form.description?.trim() || undefined,
    retention_days: Number(form.retention_days),
    folder_pattern: form.folder_pattern?.trim() || undefined,
    apply_to_all: !!form.apply_to_all,
  };
}

export function RetentionTab() {
  const qc = useQueryClient();
  const listQ = useQuery<RetentionPolicy[]>({
    queryKey: ['admin', 'retention'],
    queryFn: () => adminRetentionApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<RetentionPolicy | null>(null);
  const [form, setForm] = useState<CreateRetentionPolicyRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const startEdit = (policy: RetentionPolicy) => {
    setEditing(policy);
    setForm({
      name: policy.name,
      description: policy.description ?? '',
      retention_days: policy.retention_days,
      folder_pattern: policy.folder_pattern ?? '',
      apply_to_all: policy.apply_to_all,
    });
    setShowForm(true);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateRetentionPolicyRequest) =>
      adminRetentionApi.create(toCreateBody(body)),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'retention'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateRetentionPolicyRequest }) =>
      adminRetentionApi.update(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'retention'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminRetentionApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'retention'] }),
    onError: (e: Error) => setError(e.message),
  });

  const submit = () => {
    setError(null);
    if (!form.name.trim()) {
      setError('Policy name is required.');
      return;
    }
    if (!form.retention_days || form.retention_days <= 0) {
      setError('Retention days must be a positive integer.');
      return;
    }
    if (editing) {
      const updateBody: UpdateRetentionPolicyRequest = {
        name: form.name.trim(),
        description: form.description?.trim() || undefined,
        retention_days: Number(form.retention_days),
        folder_pattern: form.folder_pattern?.trim() || undefined,
        apply_to_all: !!form.apply_to_all,
      };
      updateMut.mutate({ id: editing.id, body: updateBody });
    } else {
      createMut.mutate(form);
    }
  };

  return (
    <div className="space-y-4" data-testid="retention-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <Clock className="size-5" /> Retention policies
          </h2>
          <p className="text-sm text-zinc-500">
            Auto-delete emails older than the configured number of days.
            Scope to all folders or a single folder pattern.
          </p>
        </div>
        <Button
          onClick={() => { closeForm(); setShowForm(true); }}
          data-testid="retention-add-button"
        >
          <Plus className="size-4 mr-2" /> Add policy
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
        <Card className="p-6 space-y-4" data-testid="retention-form">
          <h3 className="text-lg font-medium">
            {editing ? `Edit "${editing.name}"` : 'New retention policy'}
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Policy name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="90-day Inbox cleanup"
                data-testid="retention-form-name"
              />
            </Field>
            <Field label="Retention days" required>
              <Input
                type="number"
                min={1}
                value={form.retention_days}
                onChange={(e) =>
                  setForm((p) => ({ ...p, retention_days: Number(e.target.value) }))
                }
                data-testid="retention-form-days"
              />
            </Field>
            <Field label="Folder pattern (optional)">
              <Input
                value={form.folder_pattern ?? ''}
                onChange={(e) =>
                  setForm((p) => ({ ...p, folder_pattern: e.target.value }))
                }
                placeholder="INBOX or Trash — leave empty for all folders"
              />
            </Field>
            <Field label="Apply to all folders">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={!!form.apply_to_all}
                  onChange={(e) =>
                    setForm((p) => ({ ...p, apply_to_all: e.target.checked }))
                  }
                />
                Ignore folder pattern and apply to every folder.
              </label>
            </Field>
          </div>
          <Field label="Description">
            <Textarea
              value={form.description ?? ''}
              onChange={(e) => setForm((p) => ({ ...p, description: e.target.value }))}
              rows={2}
              placeholder="Why this policy exists, e.g. SEC 17a-4 archival."
            />
          </Field>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={createMut.isPending || updateMut.isPending}
              data-testid="retention-form-submit"
            >
              {editing ? 'Save changes' : 'Create policy'}
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading retention policies…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">
            Couldn't load policies: {String(listQ.error)}
          </div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="retention-empty">
            No retention policies yet. Click "Add policy" to create one.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((p) => (
              <li
                key={p.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="retention-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{p.name}</span>
                    <Badge variant="default">{p.retention_days} days</Badge>
                    {p.apply_to_all ? (
                      <Badge variant="secondary">all folders</Badge>
                    ) : p.folder_pattern ? (
                      <Badge variant="outline">{p.folder_pattern}</Badge>
                    ) : null}
                  </div>
                  {p.description && (
                    <div className="text-xs text-zinc-500">{p.description}</div>
                  )}
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => startEdit(p)}
                    title="Edit"
                  >
                    <Edit2 className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-red-600 hover:bg-red-50 dark:hover:bg-red-950"
                    disabled={deleteMut.isPending}
                    onClick={() => {
                      if (window.confirm(`Delete retention policy "${p.name}"?`)) {
                        deleteMut.mutate(p.id);
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
