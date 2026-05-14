// TMAIL-200: domains admin page.
//
// CRUD over /api/admin/domains. The synthetic byok.tasmail row is marked
// undeletable (deleting it would break /api/auth/signup, which looks up
// that domain by name in handlers/auth.rs::signup).
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Globe, Plus, Trash2 } from 'lucide-react';
import { adminDomainsApi, type Domain } from '../../api/admin-domains';

const PROTECTED_DOMAINS = new Set(['byok.tasmail']);

export function DomainsManager() {
  const queryClient = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [newName, setNewName] = useState('');
  const [error, setError] = useState<string | null>(null);

  const list = useQuery<Domain[]>({
    queryKey: ['admin-domains'],
    queryFn: () => adminDomainsApi.list(),
  });

  const createMut = useMutation({
    mutationFn: (name: string) => adminDomainsApi.create(name),
    onSuccess: () => {
      setNewName('');
      setAdding(false);
      setError(null);
      queryClient.invalidateQueries({ queryKey: ['admin-domains'] });
    },
    onError: (err: Error) => setError(err.message || 'Could not add domain.'),
  });

  const deleteMut = useMutation({
    mutationFn: (id: string) => adminDomainsApi.delete(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin-domains'] }),
    onError: (err: Error) => setError(err.message || 'Could not delete domain.'),
  });

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <Globe size={22} /> Domains
      </h1>
      <p style={{ color: 'var(--color-text-secondary, #64748b)', marginTop: 0 }}>
        Mail domains the operator owns. Mailbox usernames must end with one of
        these (or the synthetic <code>byok.tasmail</code> domain used by BYOK
        signups).
      </p>

      <div style={{ marginBottom: 16 }}>
        {!adding && (
          <button className="btn btn--primary" onClick={() => { setAdding(true); setError(null); }}>
            <Plus size={14} /> Add domain
          </button>
        )}
        {adding && (
          <form
            onSubmit={(e) => { e.preventDefault(); if (newName.trim()) createMut.mutate(newName.trim().toLowerCase()); }}
            style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap' }}
          >
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="example.com"
              autoFocus
              style={{ padding: '6px 10px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)', minWidth: 240 }}
            />
            <button type="submit" className="btn btn--primary" disabled={createMut.isPending || !newName.trim()}>
              {createMut.isPending ? 'Adding…' : 'Save'}
            </button>
            <button type="button" className="btn btn--ghost" onClick={() => { setAdding(false); setNewName(''); setError(null); }} disabled={createMut.isPending}>
              Cancel
            </button>
          </form>
        )}
        {error && (
          <div role="alert" style={{ marginTop: 8, color: 'var(--color-danger, #dc2626)', fontSize: 13 }}>{error}</div>
        )}
      </div>

      {list.isLoading && <p>Loading domains…</p>}
      {list.isError && (
        <p style={{ color: 'var(--color-danger, #dc2626)' }}>{(list.error as Error)?.message ?? 'unknown error'}</p>
      )}
      {list.data && (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Domain</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Active</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Created</th>
              <th style={{ textAlign: 'right', padding: '8px 12px' }}></th>
            </tr>
          </thead>
          <tbody>
            {list.data.length === 0 && (
              <tr><td colSpan={4} style={{ padding: 24, textAlign: 'center', color: 'var(--color-text-secondary, #64748b)' }}>No domains configured.</td></tr>
            )}
            {list.data.map((d) => {
              const isProtected = PROTECTED_DOMAINS.has(d.name);
              return (
                <tr key={d.id} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace' }}>
                    {d.name}
                    {isProtected && (
                      <span style={{ marginLeft: 8, padding: '1px 6px', background: 'var(--color-bg-elevated, #f1f5f9)', borderRadius: 4, fontSize: 11, color: 'var(--color-text-secondary, #64748b)' }}>
                        protected
                      </span>
                    )}
                  </td>
                  <td style={{ padding: '8px 12px' }}>{d.active ? 'Yes' : 'No'}</td>
                  <td style={{ padding: '8px 12px', color: 'var(--color-text-secondary, #64748b)' }}>{new Date(d.created_at).toLocaleDateString()}</td>
                  <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                    <button
                      className="btn btn--icon btn--danger"
                      onClick={() => {
                        if (isProtected) return;
                        if (confirm(`Delete domain "${d.name}"? This will fail if any mailboxes still reference it.`)) {
                          deleteMut.mutate(d.id);
                        }
                      }}
                      disabled={isProtected || deleteMut.isPending}
                      title={isProtected ? 'Protected — cannot be deleted' : 'Delete domain'}
                      aria-label={`Delete ${d.name}`}
                    >
                      <Trash2 size={16} />
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
