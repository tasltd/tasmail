// TMAIL-353: Modern UI Admin → LDAP/AD sources sub-tab. Lists directory
// sources, lets admins CRUD, run the bind-check via POST /admin/ldap/{id}/test,
// trigger a manual sync via POST /admin/ldap/{id}/sync, and surface the
// last_sync_status + users_synced metadata so admins can see whether
// the scheduled sync is healthy without digging into logs.
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, Trash2, Edit2, FlaskConical, RefreshCw, AlertCircle,
  CheckCircle, Server,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Badge } from '@/components/ui/badge';
import {
  adminLdapApi,
  type LdapConfiguration,
  type CreateLdapConfigRequest,
  type UpdateLdapConfigRequest,
} from '@/api/admin-ldap';

const EMPTY_CREATE: CreateLdapConfigRequest = {
  name: '',
  server_url: '',
  bind_dn: '',
  bind_password: '',
  search_base: '',
  search_filter: '(objectClass=person)',
  email_attribute: 'mail',
  name_attribute: 'displayName',
  sync_interval_minutes: 60,
};

function formatDate(iso: string | null): string {
  if (!iso) return '—';
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

export function LdapTab() {
  const qc = useQueryClient();
  const listQ = useQuery<LdapConfiguration[]>({
    queryKey: ['admin', 'ldap'],
    queryFn: () => adminLdapApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<LdapConfiguration | null>(null);
  const [form, setForm] = useState<CreateLdapConfigRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);
  const [rowMessage, setRowMessage] = useState<{ id: string; ok: boolean; message: string } | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setEditing(null);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const startEdit = (cfg: LdapConfiguration) => {
    setEditing(cfg);
    setForm({
      name: cfg.name,
      server_url: cfg.server_url,
      bind_dn: cfg.bind_dn,
      // Bind password isn't returned by the API — leave blank, only send
      // when the admin rotates it.
      bind_password: '',
      search_base: cfg.search_base,
      search_filter: cfg.search_filter,
      email_attribute: cfg.email_attribute,
      name_attribute: cfg.name_attribute,
      group_filter: cfg.group_filter ?? '',
      sync_interval_minutes: cfg.sync_interval_minutes,
    });
    setShowForm(true);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateLdapConfigRequest) => adminLdapApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'ldap'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateLdapConfigRequest }) =>
      adminLdapApi.update(id, body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'ldap'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const deleteMut = useMutation({
    mutationFn: (id: string) => adminLdapApi.delete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'ldap'] }),
    onError: (e: Error) => setError(e.message),
  });
  const testMut = useMutation({
    mutationFn: (id: string) => adminLdapApi.test(id),
    onSuccess: (_data, id) =>
      setRowMessage({ id, ok: true, message: 'Bind succeeded — credentials work.' }),
    onError: (e: Error, id) =>
      setRowMessage({ id, ok: false, message: e.message }),
  });
  const syncMut = useMutation({
    mutationFn: (id: string) => adminLdapApi.sync(id),
    onSuccess: (log, id) => {
      qc.invalidateQueries({ queryKey: ['admin', 'ldap'] });
      setRowMessage({
        id,
        ok: log.status === 'success',
        message: `Sync ${log.status}: +${log.users_created} created, ${log.users_updated} updated, ${log.users_disabled} disabled.`,
      });
    },
    onError: (e: Error, id) =>
      setRowMessage({ id, ok: false, message: e.message }),
  });

  const submit = () => {
    setError(null);
    if (editing) {
      const body: UpdateLdapConfigRequest = { ...form };
      if (!form.bind_password) delete body.bind_password;
      updateMut.mutate({ id: editing.id, body });
    } else {
      createMut.mutate(form);
    }
  };

  return (
    <div className="space-y-4" data-testid="ldap-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <Server className="size-5" /> LDAP / Active Directory sources
          </h2>
          <p className="text-sm text-zinc-500">
            Pull mailbox identities from an upstream directory on a schedule.
          </p>
        </div>
        <Button onClick={() => { closeForm(); setShowForm(true); }} data-testid="ldap-add-button">
          <Plus className="size-4 mr-2" /> Add source
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
        <Card className="p-6 space-y-4" data-testid="ldap-form">
          <h3 className="text-lg font-medium">
            {editing ? `Edit "${editing.name}"` : 'New LDAP source'}
          </h3>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Field label="Display name" required>
              <Input
                value={form.name}
                onChange={(e) => setForm((p) => ({ ...p, name: e.target.value }))}
                placeholder="Acme Active Directory"
                data-testid="ldap-form-name"
              />
            </Field>
            <Field label="Server URL" required>
              <Input
                value={form.server_url}
                onChange={(e) => setForm((p) => ({ ...p, server_url: e.target.value }))}
                placeholder="ldaps://dc.acme.local:636"
              />
            </Field>
            <Field label="Bind DN" required>
              <Input
                value={form.bind_dn}
                onChange={(e) => setForm((p) => ({ ...p, bind_dn: e.target.value }))}
                placeholder="CN=Service Account,OU=Service Accounts,DC=acme,DC=local"
              />
            </Field>
            <Field label={editing ? 'Bind password (leave blank to keep)' : 'Bind password'} required={!editing}>
              <Input
                type="password"
                value={form.bind_password}
                onChange={(e) => setForm((p) => ({ ...p, bind_password: e.target.value }))}
                placeholder="••••••••"
              />
            </Field>
            <Field label="Search base" required>
              <Input
                value={form.search_base}
                onChange={(e) => setForm((p) => ({ ...p, search_base: e.target.value }))}
                placeholder="OU=Users,DC=acme,DC=local"
              />
            </Field>
            <Field label="Search filter">
              <Input
                value={form.search_filter ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, search_filter: e.target.value }))}
                placeholder="(objectClass=person)"
              />
            </Field>
            <Field label="Email attribute">
              <Input
                value={form.email_attribute ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, email_attribute: e.target.value }))}
                placeholder="mail"
              />
            </Field>
            <Field label="Display-name attribute">
              <Input
                value={form.name_attribute ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, name_attribute: e.target.value }))}
                placeholder="displayName"
              />
            </Field>
            <Field label="Group filter (optional)">
              <Input
                value={form.group_filter ?? ''}
                onChange={(e) => setForm((p) => ({ ...p, group_filter: e.target.value }))}
                placeholder="(memberOf=CN=TASMail Users,…)"
              />
            </Field>
            <Field label="Sync interval (minutes)">
              <Input
                type="number"
                min={1}
                value={form.sync_interval_minutes ?? 60}
                onChange={(e) =>
                  setForm((p) => ({
                    ...p,
                    sync_interval_minutes: Number.parseInt(e.target.value, 10) || 60,
                  }))
                }
              />
            </Field>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={
                !form.name ||
                !form.server_url ||
                !form.bind_dn ||
                !form.search_base ||
                (!editing && !form.bind_password) ||
                createMut.isPending ||
                updateMut.isPending
              }
              data-testid="ldap-form-submit"
            >
              {editing ? 'Save changes' : 'Create source'}
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading LDAP sources…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">Couldn't load sources: {String(listQ.error)}</div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="ldap-empty">
            No directory sources yet. Click "Add source" to connect a directory.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((cfg) => (
              <li
                key={cfg.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="ldap-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="font-medium">{cfg.name}</span>
                    <Badge variant={cfg.active ? 'default' : 'secondary'}>
                      {cfg.active ? 'active' : 'inactive'}
                    </Badge>
                    {cfg.last_sync_status && (
                      <Badge variant={cfg.last_sync_status === 'success' ? 'default' : 'destructive'}>
                        last sync: {cfg.last_sync_status}
                      </Badge>
                    )}
                  </div>
                  <div className="text-xs text-zinc-500 font-mono break-all">{cfg.server_url}</div>
                  <div className="text-xs text-zinc-500">
                    base: <span className="font-mono">{cfg.search_base}</span>
                  </div>
                  <div className="text-xs text-zinc-500">
                    Last sync: {formatDate(cfg.last_sync_at)} • users synced: {cfg.users_synced ?? 0} • every {cfg.sync_interval_minutes} min
                  </div>
                  {rowMessage?.id === cfg.id && (
                    <div
                      className={`text-xs flex items-start gap-1 mt-1 ${
                        rowMessage.ok ? 'text-green-700 dark:text-green-300' : 'text-red-700 dark:text-red-300'
                      }`}
                      data-testid="ldap-row-message"
                    >
                      {rowMessage.ok ? <CheckCircle className="size-3 mt-0.5" /> : <AlertCircle className="size-3 mt-0.5" />}
                      <span className="break-all">{rowMessage.message}</span>
                    </div>
                  )}
                </div>
                <div className="flex gap-1 shrink-0">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => testMut.mutate(cfg.id)}
                    disabled={testMut.isPending && testMut.variables === cfg.id}
                    title="Test bind"
                    data-testid="ldap-test-button"
                  >
                    <FlaskConical className="size-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => syncMut.mutate(cfg.id)}
                    disabled={syncMut.isPending && syncMut.variables === cfg.id}
                    title="Sync now"
                    data-testid="ldap-sync-button"
                  >
                    <RefreshCw className={`size-4 ${syncMut.isPending && syncMut.variables === cfg.id ? 'animate-spin' : ''}`} />
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
                      if (window.confirm(`Delete LDAP source "${cfg.name}"?`)) {
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
