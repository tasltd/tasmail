// TMAIL-202: users admin page.
//
// Lists every mailbox across every domain, lets the operator create or
// delete one, and accepts a CSV bulk-import that drives /api/admin/users/
// bulk-import (multipart). Domains for the create form are pulled from the
// existing /admin/domains client (TMAIL-200 already shipped that).
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Users, Plus, Trash2, Upload, ShieldCheck } from 'lucide-react';
import { adminUsersApi, type CreateUserRequest, type UserInfo, type BulkImportResult } from '../../api/admin-users';
import { adminDomainsApi, type Domain } from '../../api/admin-domains';

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function UsersManager() {
  const queryClient = useQueryClient();
  const [creating, setCreating] = useState(false);
  const [form, setForm] = useState<CreateUserRequest>({ username: '', password: '', domain_id: '' });
  const [error, setError] = useState<string | null>(null);
  const [bulkResult, setBulkResult] = useState<BulkImportResult | null>(null);

  const users = useQuery<UserInfo[]>({
    queryKey: ['admin-users'],
    queryFn: () => adminUsersApi.list(),
  });
  const domains = useQuery<Domain[]>({
    queryKey: ['admin-domains'],
    queryFn: () => adminDomainsApi.list(),
  });

  const createMut = useMutation({
    mutationFn: (body: CreateUserRequest) => adminUsersApi.create(body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-users'] });
      setCreating(false);
      setForm({ username: '', password: '', domain_id: '' });
      setError(null);
    },
    onError: (err: Error) => setError(err.message || 'Could not create user.'),
  });

  const deleteMut = useMutation({
    mutationFn: (id: string) => adminUsersApi.delete(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin-users'] }),
    onError: (err: Error) => setError(err.message || 'Could not delete user.'),
  });

  const bulkMut = useMutation({
    mutationFn: (file: File) => adminUsersApi.bulkImport(file),
    onSuccess: (res) => {
      setBulkResult(res);
      queryClient.invalidateQueries({ queryKey: ['admin-users'] });
    },
    onError: (err: Error) => setError(err.message || 'Bulk import failed.'),
  });

  function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!form.domain_id) { setError('Pick a domain.'); return; }
    if (form.password.length < 8) { setError('Password must be at least 8 characters.'); return; }
    createMut.mutate(form);
  }

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <Users size={22} /> Users
      </h1>

      <div style={{ display: 'flex', gap: 12, alignItems: 'center', marginBottom: 16, flexWrap: 'wrap' }}>
        {!creating && (
          <button className="btn btn--primary" onClick={() => { setCreating(true); setError(null); }}>
            <Plus size={14} /> Add user
          </button>
        )}
        <label className="btn btn--ghost" style={{ cursor: 'pointer' }}>
          <Upload size={14} /> Bulk import (CSV)
          <input
            type="file"
            accept=".csv,text/csv"
            style={{ display: 'none' }}
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) bulkMut.mutate(file);
              e.target.value = '';
            }}
          />
        </label>
        {bulkMut.isPending && <span style={{ fontSize: 13 }}>Uploading…</span>}
      </div>

      {creating && (
        <form onSubmit={submit} style={{ background: 'var(--color-bg-elevated, #f8fafc)', border: '1px solid var(--color-border, #e5e7eb)', padding: 16, borderRadius: 8, maxWidth: 560, marginBottom: 16 }}>
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
            <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
              <span style={{ marginBottom: 4 }}>Email / username</span>
              <input value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value.toLowerCase() })} placeholder="alice@example.com" autoFocus style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }} />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
              <span style={{ marginBottom: 4 }}>Domain</span>
              <select value={form.domain_id} onChange={(e) => setForm({ ...form, domain_id: e.target.value })} style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }}>
                <option value="">— pick a domain —</option>
                {(domains.data ?? []).map((d) => (<option key={d.id} value={d.id}>{d.name}</option>))}
              </select>
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
              <span style={{ marginBottom: 4 }}>Display name (optional)</span>
              <input value={form.display_name ?? ''} onChange={(e) => setForm({ ...form, display_name: e.target.value })} style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }} />
            </label>
            <label style={{ display: 'flex', flexDirection: 'column', fontSize: 12 }}>
              <span style={{ marginBottom: 4 }}>Password (≥ 8 chars)</span>
              <input type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} autoComplete="new-password" style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }} />
            </label>
          </div>
          {error && <div role="alert" style={{ color: 'var(--color-danger, #dc2626)', fontSize: 13, marginTop: 8 }}>{error}</div>}
          <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
            <button type="submit" className="btn btn--primary" disabled={createMut.isPending}>{createMut.isPending ? 'Creating…' : 'Create user'}</button>
            <button type="button" className="btn btn--ghost" onClick={() => { setCreating(false); setError(null); }} disabled={createMut.isPending}>Cancel</button>
          </div>
        </form>
      )}

      {bulkResult && (
        <div role="status" style={{ background: 'var(--color-bg-elevated, #f8fafc)', border: '1px solid var(--color-border, #e5e7eb)', padding: 12, borderRadius: 6, marginBottom: 16, fontSize: 13 }}>
          Bulk import <code>{bulkResult.filename}</code>: {bulkResult.success_count} of {bulkResult.total_rows} rows imported,
          {' '}{bulkResult.error_count} errors. Status: <strong>{bulkResult.status}</strong>.
        </div>
      )}
      {error && !creating && (
        <div role="alert" style={{ color: 'var(--color-danger, #dc2626)', fontSize: 13, marginBottom: 12 }}>{error}</div>
      )}

      {users.isLoading && <p>Loading users…</p>}
      {users.data && (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Username</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Display name</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Quota</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Active</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Admin</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Created</th>
              <th style={{ textAlign: 'right', padding: '8px 12px' }}></th>
            </tr>
          </thead>
          <tbody>
            {users.data.length === 0 && (
              <tr><td colSpan={7} style={{ padding: 24, textAlign: 'center', color: 'var(--color-text-secondary, #64748b)' }}>No users yet.</td></tr>
            )}
            {users.data.map((u) => (
              <tr key={u.id} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: 12 }}>{u.username}</td>
                <td style={{ padding: '8px 12px' }}>{u.display_name ?? '—'}</td>
                <td style={{ padding: '8px 12px' }}>{formatBytes(u.quota_bytes)}</td>
                <td style={{ padding: '8px 12px' }}>{u.active ? 'Yes' : 'No'}</td>
                <td style={{ padding: '8px 12px' }}>{u.is_admin ? <ShieldCheck size={14} color="#22c55e" /> : '—'}</td>
                <td style={{ padding: '8px 12px', color: 'var(--color-text-secondary, #64748b)' }}>{new Date(u.created_at).toLocaleDateString()}</td>
                <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                  <button
                    className="btn btn--icon btn--danger"
                    onClick={() => {
                      if (confirm(`Delete ${u.username}? Their mail metadata (signatures, sessions, contacts, push devices, etc.) will cascade-delete.`)) {
                        deleteMut.mutate(u.id);
                      }
                    }}
                    disabled={deleteMut.isPending}
                    title="Delete user"
                    aria-label={`Delete ${u.username}`}
                  >
                    <Trash2 size={16} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
