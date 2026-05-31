// TMAIL-353: Modern UI Admin → SAML providers sub-tab. Lists configured
// SAML 2.0 IdPs, lets admins create / edit / delete and probe with the
// "Test" button (fetches the IdP redirect URL via GET /api/auth/saml/{id}/login).
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Edit2, FlaskConical, AlertCircle, CheckCircle, ShieldCheck,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import {
  adminSamlApi,
  type SamlConfiguration,
  type CreateSamlConfigRequest,
  type UpdateSamlConfigRequest,
} from '@/api/admin-saml';

const EMPTY_CREATE: CreateSamlConfigRequest = {
  name: '',
  entity_id: '',
  sso_url: '',
  slo_url: '',
  certificate: '',
  name_id_format: 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress',
  auto_create_users: true,
};

export function SamlTab() {
  const qc = useQueryClient();
  const listQ = useQuery<SamlConfiguration[]>({
    queryKey: ['admin', 'saml'],
    queryFn: () => adminSamlApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<SamlConfiguration | null>(null);
  const [form, setForm] = useState<CreateSamlConfigRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<{ id: string; ok: boolean; message: string } | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const startEdit = (cfg: SamlConfiguration) => {
    setEditing(cfg);
    setForm({
      name: cfg.name,
      entity_id: cfg.entity_id,
      sso_url: cfg.sso_url,
      slo_url: cfg.slo_url ?? '',
      certificate: cfg.certificate,
      name_id_format: cfg.name_id_format,
      auto_create_users: cfg.auto_create_users,
    });
    setShowForm(true);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateSamlConfigRequest) => adminSamlApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'saml'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateSamlConfigRequest }) =>
      adminSamlApi.update(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'saml'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminSamlApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'saml'] }),
    onError: (e: Error) => setError(e.message),
  });

  const testMut = useMutation({
    mutationFn: (id: string) => adminSamlApi.test(id),
    onSuccess: (data, id) =>
      setTestResult({
        id,
        ok: true,
        message: `IdP redirect URL: ${data.redirect_url}`,
      }),
    onError: (e: Error, id) => setTestResult({ id, ok: false, message: e.message }),
  });

  const submit = () => {
    setError(null);
    if (editing) {
      updateMut.mutate({ id: editing.id, body: form });
    } else {
      createMut.mutate(form);
    }
  };

  return (
    <div className="space-y-4" data-testid="saml-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <ShieldCheck className="size-5" /> SAML 2.0 providers
          </h2>
          <p className="text-sm text-zinc-500">
            Configure enterprise SSO via SAML IdPs (Okta, Azure AD, OneLogin, etc.).
          </p>
        </div>
        <Button onClick={() => { closeForm(); setShowForm(true); }} data-testid="saml-add-button">
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
        <Card className="p-6 space-y-4" data-testid="saml-form">
          <h3 className="text-lg font-medium">
            {editing ? `Edit "${editing.name}"` : 'New SAML provider'}
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Display name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="Acme Okta"
                data-testid="saml-form-name"
              />
            </Field>
            <Field label="IdP Entity ID" required>
              <Input
                value={form.entity_id}
                onChange={(e) => setForm((p) => ({ ...p, entity_id: e.target.value }))}
                placeholder="https://idp.acme.com/saml/metadata"
              />
            </Field>
            <Field label="SSO URL" required>
              <Input
                value={form.sso_url}
                onChange={(e) => setForm((p) => ({ ...p, sso_url: e.target.value }))}
                placeholder="https://idp.acme.com/saml/sso"
              />
            </Field>
            <Field label="SLO URL (optional)">
              <Input
                value={form.slo_url ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, slo_url: e.target.value }))}
                placeholder="https://idp.acme.com/saml/slo"
              />
            </Field>
            <Field label="NameID format">
              <Input
                value={form.name_id_format ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, name_id_format: e.target.value }))}
              />
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
                When ON, unknown SAML subjects get a mailbox created on first login.
              </label>
            </Field>
          </div>
          <Field label="X.509 certificate (PEM)" required>
            <Textarea
              value={form.certificate}
              onChange={(e) => setForm((p) => ({ ...p, certificate: e.target.value }))}
              rows={8}
              className="font-mono text-xs"
              placeholder="-----BEGIN CERTIFICATE-----&#10;…&#10;-----END CERTIFICATE-----"
            />
          </Field>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={
                !form.name ||
                !form.entity_id ||
                !form.sso_url ||
                !form.certificate ||
                createMut.isPending ||
                updateMut.isPending
              }
              data-testid="saml-form-submit"
            >
              {editing ? 'Save changes' : 'Create provider'}
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading SAML providers…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">Couldn't load providers: {String(listQ.error)}</div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="saml-empty">
            No SAML providers configured yet. Click "Add provider" to wire one up.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((cfg) => (
              <li
                key={cfg.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="saml-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{cfg.name}</span>
                    <Badge variant={cfg.active ? 'default' : 'secondary'}>
                      {cfg.active ? 'active' : 'inactive'}
                    </Badge>
                  </div>
                  <div className="text-xs text-zinc-500 font-mono break-all">
                    {cfg.entity_id}
                  </div>
                  <div className="text-xs text-zinc-500">
                    SSO: <span className="font-mono">{cfg.sso_url}</span>
                  </div>
                  {testResult?.id === cfg.id && (
                    <div
                      className={`text-xs flex items-start gap-1 mt-1 ${
                        testResult.ok ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300'
                      }`}
                      data-testid="saml-test-result"
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
                    onClick={() => testMut.mutate(cfg.id)}
                    disabled={testMut.isPending && testMut.variables === cfg.id}
                    title="Test connection"
                    data-testid="saml-test-button"
                  >
                    <FlaskConical className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => startEdit(cfg)}
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
                      if (window.confirm(`Delete SAML provider "${cfg.name}"?`)) {
                        deleteMut.mutate(cfg.id);
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
