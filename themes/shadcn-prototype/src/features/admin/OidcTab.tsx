// TMAIL-353: Modern UI Admin → OIDC providers sub-tab. Lists Google /
// Microsoft / generic OIDC IdPs, lets admins CRUD and probe with "Test"
// (fetches an authorize URL via GET /api/auth/oidc/{id}/authorize — proves
// the discovery doc is reachable and the client_id is recognised).
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Edit2, FlaskConical, AlertCircle, CheckCircle, KeyRound,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import {
  adminOidcApi,
  type OidcProvider,
  type CreateOidcProviderRequest,
  type UpdateOidcProviderRequest,
} from '@/api/admin-oidc';

const EMPTY_CREATE: CreateOidcProviderRequest = {
  name: '',
  issuer_url: '',
  client_id: '',
  client_secret: '',
  redirect_uri: '',
  scopes: 'openid email profile',
  auto_create_users: true,
  default_role: 'user',
};

export function OidcTab() {
  const qc = useQueryClient();
  const listQ = useQuery<OidcProvider[]>({
    queryKey: ['admin', 'oidc'],
    queryFn: () => adminOidcApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<OidcProvider | null>(null);
  const [form, setForm] = useState<CreateOidcProviderRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{ id: string; ok: boolean; message: string } | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const startEdit = (p: OidcProvider) => {
    setEditing(p);
    setForm({
      name: p.name,
      issuer_url: p.issuer_url,
      client_id: p.client_id,
      // Backend never returns the encrypted secret — leave blank on edit
      // and only send if the admin types a new value.
      client_secret: '',
      redirect_uri: p.redirect_uri,
      scopes: p.scopes,
      auto_create_users: p.auto_create_users,
      default_role: p.default_role,
      icon_url: p.icon_url ?? '',
      button_label: p.button_label ?? '',
    });
    setShowForm(true);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateOidcProviderRequest) => adminOidcApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'oidc'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateOidcProviderRequest }) =>
      adminOidcApi.update(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'oidc'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminOidcApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'oidc'] }),
    onError: (e: Error) => setError(e.message),
  });
  const testMut = useMutation({
    mutationFn: (id: string) => adminOidcApi.test(id),
    onSuccess: (data, id) =>
      setTestResult({ id, ok: true, message: `Authorize URL OK: ${data.authorize_url}` }),
    onError: (e: Error, id) => setTestResult({ id, ok: false, message: e.message }),
  });

  const submit = () => {
    setError(null);
    if (editing) {
      // Build update body — skip client_secret if untouched so we don't
      // overwrite the stored cipher with the empty string.
      const body: UpdateOidcProviderRequest = { ...form };
      if (!form.client_secret) delete body.client_secret;
      updateMut.mutate({ id: editing.id, body });
    } else {
      createMut.mutate(form);
    }
  };

  return (
    <div className="space-y-4" data-testid="oidc-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <KeyRound className="size-5" /> OIDC providers
          </h2>
          <p className="text-sm text-zinc-500">
            Configure OIDC IdPs (Google Workspace, Microsoft Entra, Auth0, Keycloak).
          </p>
        </div>
        <Button onClick={() => { closeForm(); setShowForm(true); }} data-testid="oidc-add-button">
          <Plus className="size-4 mr-2" /> Add provider
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
        <Card className="p-6 space-y-4" data-testid="oidc-form">
          <h3 className="text-lg font-medium">
            {editing ? `Edit "${editing.name}"` : 'New OIDC provider'}
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Display name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="Google Workspace"
                data-testid="oidc-form-name"
              />
            </Field>
            <Field label="Issuer URL" required>
              <Input
                value={form.issuer_url}
                onChange={(e) => setForm((p) => ({ ...p, issuer_url: e.target.value }))}
                placeholder="https://accounts.google.com"
              />
            </Field>
            <Field label="Client ID" required>
              <Input
                value={form.client_id}
                onChange={(e) => setForm((p) => ({ ...p, client_id: e.target.value }))}
              />
            </Field>
            <Field label={editing ? 'Client secret (leave blank to keep)' : 'Client secret'} required={!editing}>
              <Input
                type="password"
                value={form.client_secret}
                onChange={(e) => setForm((p) => ({ ...p, client_secret: e.target.value }))}
                placeholder="••••••••"
              />
            </Field>
            <Field label="Redirect URI" required>
              <Input
                value={form.redirect_uri}
                onChange={(e) => setForm((p) => ({ ...p, redirect_uri: e.target.value }))}
                placeholder="https://mail.example.com/api/auth/oidc/callback"
              />
            </Field>
            <Field label="Scopes">
              <Input
                value={form.scopes ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, scopes: e.target.value }))}
                placeholder="openid email profile"
              />
            </Field>
            <Field label="Default role">
              <select
                className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm"
                value={form.default_role ?? 'user'}
                onChange={(e) => setForm((p) => ({ ...p, default_role: e.target.value }))}
              >
                <option value="user">user</option>
                <option value="admin">admin</option>
              </select>
            </Field>
            <Field label="Auto-provision users">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={form.auto_create_users ?? true}
                  onChange={(e) =>
                    setForm((p) => ({ ...p, auto_create_users: e.target.checked }))
                  }
                />
                When ON, unknown OIDC subjects get a mailbox created on first login.
              </label>
            </Field>
            <Field label="Button label (optional)">
              <Input
                value={form.button_label ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, button_label: e.target.value }))}
                placeholder="Sign in with Google"
              />
            </Field>
            <Field label="Icon URL (optional)">
              <Input
                value={form.icon_url ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, icon_url: e.target.value }))}
                placeholder="https://cdn.example.com/google-icon.svg"
              />
            </Field>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={
                !form.name ||
                !form.issuer_url ||
                !form.client_id ||
                !form.redirect_uri ||
                (!editing && !form.client_secret) ||
                createMut.isPending ||
                updateMut.isPending
              }
              data-testid="oidc-form-submit"
            >
              {editing ? 'Save changes' : 'Create provider'}
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading OIDC providers…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">Couldn't load providers: {String(listQ.error)}</div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="oidc-empty">
            No OIDC providers configured yet. Click "Add provider" to wire one up.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((p) => (
              <li
                key={p.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="oidc-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{p.name}</span>
                    <Badge variant={p.active ? 'default' : 'secondary'}>
                      {p.active ? 'active' : 'inactive'}
                    </Badge>
                  </div>
                  <div className="text-xs text-zinc-500 font-mono break-all">{p.issuer_url}</div>
                  <div className="text-xs text-zinc-500">client_id: <span className="font-mono">{p.client_id}</span></div>
                  {testResult?.id === p.id && (
                    <div
                      className={`text-xs flex items-start gap-1 mt-1 ${
                        testResult.ok ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300'
                      }`}
                      data-testid="oidc-test-result"
                    >
                      {testResult.ok ? <CheckCircle className="size-3 mt-0.5" /> : <AlertCircle className="size-3 mt-0.5" />}
                      <span className="break-all">{testResult.message}</span>
                    </div>
                  )}
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => testMut.mutate(p.id)}
                    disabled={testMut.isPending && testMut.variables === p.id}
                    title="Test connection"
                    data-testid="oidc-test-button"
                  >
                    <FlaskConical className="size-4" />
                  </Button>
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
                      if (window.confirm(`Delete OIDC provider "${p.name}"?`)) {
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
