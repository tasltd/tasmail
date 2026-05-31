// TMAIL-354: Modern UI Admin → Legal Holds sub-tab. Place a legal hold on
// a mailbox to bypass retention auto-deletion (compliance / litigation),
// or release an existing hold. List shows active and historical holds
// alongside the operator who placed each and the reason.
//
// Backed by /api/admin/legal-holds (list, create, release). Mailboxes
// come from /api/admin/users so the create form can show usernames
// instead of opaque UUIDs.
import { useMemo, useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Plus, AlertCircle, Scale, ShieldCheck, ShieldOff, CheckCircle,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Badge } from '@/components/ui/badge';
import {
  adminLegalHoldsApi,
  type LegalHold,
  type CreateLegalHoldRequest,
} from '@/api/admin-legal-holds';
import { adminUsersApi, type UserInfo } from '@/api/admin-users';
import { adminDomainsApi, type Domain } from '@/api/admin-domains';

const EMPTY_CREATE: CreateLegalHoldRequest = {
  user_id: '',
  reason: '',
};

export function LegalHoldsTab() {
  const qc = useQueryClient();
  const listQ = useQuery<LegalHold[]>({
    queryKey: ['admin', 'legal-holds'],
    queryFn: () => adminLegalHoldsApi.list(),
  });
  // We need the user + domain list so the form can pick a mailbox by
  // username instead of forcing the operator to copy a UUID. Both lists
  // already cache-share with the other admin tabs.
  const usersQ = useQuery<UserInfo[]>({
    queryKey: ['admin', 'users'],
    queryFn: () => adminUsersApi.list(),
  });
  const domainsQ = useQuery<Domain[]>({
    queryKey: ['admin', 'domains'],
    queryFn: () => adminDomainsApi.list(),
  });

  const [showForm, setShowForm] = useState(false);
  const [form, setForm] = useState<CreateLegalHoldRequest>(EMPTY_CREATE);
  const [error, setError] = useState<string | null>(null);

  const closeForm = () => {
    setShowForm(false);
    setForm(EMPTY_CREATE);
    setError(null);
  };

  const createMut = useMutation({
    mutationFn: (body: CreateLegalHoldRequest) => adminLegalHoldsApi.create(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['admin', 'legal-holds'] });
      closeForm();
    },
    onError: (e: Error) => setError(e.message),
  });
  const releaseMut = useMutation({
    mutationFn: (id: string) => adminLegalHoldsApi.release(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['admin', 'legal-holds'] }),
    onError: (e: Error) => setError(e.message),
  });

  // Index users by id for the row → username lookup.
  const userById = useMemo(() => {
    const map = new Map<string, UserInfo>();
    (usersQ.data ?? []).forEach((u) => map.set(u.id, u));
    return map;
  }, [usersQ.data]);
  const domainById = useMemo(() => {
    const map = new Map<string, Domain>();
    (domainsQ.data ?? []).forEach((d) => map.set(d.id, d));
    return map;
  }, [domainsQ.data]);

  const formatMailbox = (userId: string): string => {
    const u = userById.get(userId);
    if (!u) return userId; // fall back to raw UUID if user record is gone.
    const d = domainById.get(u.domain_id);
    return d ? `${u.username}@${d.name}` : u.username;
  };

  const submit = () => {
    setError(null);
    if (!form.user_id) {
      setError('Pick a mailbox to place on hold.');
      return;
    }
    if (!form.reason.trim()) {
      setError('A reason is required for the audit trail.');
      return;
    }
    createMut.mutate({ user_id: form.user_id, reason: form.reason.trim() });
  };

  return (
    <div className="space-y-4" data-testid="legal-holds-tab">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-xl font-semibold flex items-center gap-2">
            <Scale className="size-5" /> Legal holds
          </h2>
          <p className="text-sm text-zinc-500">
            Suspend retention-driven deletion on specific mailboxes for
            litigation, audit, or compliance investigations. Every
            placement and release is audit-logged.
          </p>
        </div>
        <Button
          onClick={() => { closeForm(); setShowForm(true); }}
          data-testid="legal-holds-add-button"
        >
          <Plus className="size-4 mr-2" /> Place hold
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
        <Card className="p-6 space-y-4" data-testid="legal-holds-form">
          <h3 className="text-lg font-medium">New legal hold</h3>
          <Field label="Mailbox" required>
            <select
              className="h-9 rounded-md border border-zinc-200 dark:border-zinc-800 bg-transparent px-3 text-sm w-full"
              value={form.user_id}
              onChange={(e) => setForm((p) => ({ ...p, user_id: e.target.value }))}
              data-testid="legal-holds-form-user"
            >
              <option value="">— pick a mailbox —</option>
              {(usersQ.data ?? []).map((u) => {
                const d = domainById.get(u.domain_id);
                return (
                  <option key={u.id} value={u.id}>
                    {u.username}{d ? `@${d.name}` : ''}
                  </option>
                );
              })}
            </select>
          </Field>
          <Field label="Reason" required>
            <Textarea
              value={form.reason}
              onChange={(e) => setForm((p) => ({ ...p, reason: e.target.value }))}
              rows={3}
              placeholder="Case #2026-001 — preserve all email pending discovery."
              data-testid="legal-holds-form-reason"
            />
          </Field>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={closeForm}>Cancel</Button>
            <Button
              onClick={submit}
              disabled={createMut.isPending}
              data-testid="legal-holds-form-submit"
            >
              Place hold
            </Button>
          </div>
        </Card>
      )}

      <Card className="p-6">
        {listQ.isLoading ? (
          <div className="text-sm text-zinc-500">Loading legal holds…</div>
        ) : listQ.isError ? (
          <div className="text-sm text-red-600">
            Couldn't load legal holds: {String(listQ.error)}
          </div>
        ) : (listQ.data ?? []).length === 0 ? (
          <div className="text-sm text-zinc-500" data-testid="legal-holds-empty">
            No legal holds on record. Click "Place hold" to create one.
          </div>
        ) : (
          <ul className="space-y-3">
            {(listQ.data ?? []).map((h) => (
              <li
                key={h.id}
                className="flex items-start justify-between p-4 border border-zinc-200 dark:border-zinc-800 rounded-lg"
                data-testid="legal-holds-row"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    {h.active ? (
                      <ShieldCheck className="size-4 text-amber-600" />
                    ) : (
                      <ShieldOff className="size-4 text-zinc-400" />
                    )}
                    <span className="font-medium">{formatMailbox(h.user_id)}</span>
                    <Badge variant={h.active ? 'default' : 'secondary'}>
                      {h.active ? 'active' : 'released'}
                    </Badge>
                  </div>
                  <div className="text-sm text-zinc-600 dark:text-zinc-300">
                    {h.reason}
                  </div>
                  <div className="text-xs text-zinc-500">
                    Placed {new Date(h.created_at).toLocaleString()}
                    {h.released_at && (
                      <> • Released {new Date(h.released_at).toLocaleString()}</>
                    )}
                  </div>
                </div>
                <div className="flex gap-1 shrink-0">
                  {h.active ? (
                    <Button
                      variant="outline"
                      size="sm"
                      disabled={releaseMut.isPending}
                      onClick={() => {
                        if (window.confirm(`Release legal hold on ${formatMailbox(h.user_id)}?`)) {
                          releaseMut.mutate(h.id);
                        }
                      }}
                      data-testid="legal-holds-release-button"
                    >
                      Release
                    </Button>
                  ) : (
                    <span className="inline-flex items-center gap-1 text-xs text-zinc-500">
                      <CheckCircle className="size-3" /> Released
                    </span>
                  )}
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
