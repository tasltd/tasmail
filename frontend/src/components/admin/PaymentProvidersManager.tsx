// TMAIL-201: payment provider config admin page.
//
// Lists every PAYSTACK/MASTERCARD/CYBERSOURCE/BANK_TRANSFER row with the
// sensitive fields shown only as has-or-not booleans (the backend never
// echoes plaintext). New-provider form lets the operator paste fresh
// credentials per provider type — only the fields each type needs are
// shown to keep the surface focused.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { CreditCard, Plus, Trash2, Check, X } from 'lucide-react';
import {
  adminPaymentProvidersApi,
  type ProviderSummary,
  type PaymentProviderType,
  type UpsertProviderRequest,
} from '../../api/admin-payment-providers';

const PROVIDER_OPTIONS: { value: PaymentProviderType; label: string }[] = [
  { value: 'PAYSTACK', label: 'Paystack' },
  { value: 'MASTERCARD', label: 'Mastercard MPGS' },
  { value: 'CYBERSOURCE', label: 'Cybersource' },
  { value: 'BANK_TRANSFER', label: 'Bank transfer (manual)' },
];

// Which credential fields each provider type uses. Anything else stays hidden.
const CREDENTIAL_FIELDS: Record<PaymentProviderType, Array<keyof UpsertProviderRequest>> = {
  PAYSTACK: ['secret_key', 'public_key', 'webhook_secret', 'callback_url', 'currency'],
  MASTERCARD: ['merchant_id', 'api_password', 'base_url', 'currency'],
  CYBERSOURCE: ['key_id', 'shared_secret_key', 'merchant_id', 'environment'],
  BANK_TRANSFER: ['bank_details', 'notes'],
};

const FIELD_LABELS: Partial<Record<keyof UpsertProviderRequest, string>> = {
  secret_key: 'Secret key',
  public_key: 'Public key',
  webhook_secret: 'Webhook secret',
  merchant_id: 'Merchant ID',
  api_password: 'API password',
  key_id: 'Key ID',
  shared_secret_key: 'Shared secret',
  callback_url: 'Callback URL',
  base_url: 'Base URL',
  currency: 'Currency',
  environment: 'Environment',
  bank_details: 'Bank details (JSON)',
  notes: 'Notes',
};

const SECRET_FIELDS = new Set<keyof UpsertProviderRequest>([
  'secret_key', 'webhook_secret', 'api_password', 'shared_secret_key',
]);

function hasMark(yes: boolean) {
  return yes ? <Check size={14} color="#22c55e" /> : <X size={14} color="#cbd5e1" />;
}

export function PaymentProvidersManager() {
  const queryClient = useQueryClient();
  const [adding, setAdding] = useState(false);
  const [providerType, setProviderType] = useState<PaymentProviderType>('PAYSTACK');
  const [name, setName] = useState('');
  const [fields, setFields] = useState<Partial<UpsertProviderRequest>>({});
  const [error, setError] = useState<string | null>(null);

  const list = useQuery<ProviderSummary[]>({
    queryKey: ['admin-payment-providers'],
    queryFn: () => adminPaymentProvidersApi.list(),
  });

  const createMut = useMutation({
    mutationFn: (body: UpsertProviderRequest) => adminPaymentProvidersApi.create(body),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['admin-payment-providers'] });
      setAdding(false);
      setName('');
      setFields({});
      setError(null);
    },
    onError: (err: Error) => setError(err.message || 'Could not save provider.'),
  });

  const archiveMut = useMutation({
    mutationFn: (id: string) => adminPaymentProvidersApi.archive(id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['admin-payment-providers'] }),
    onError: (err: Error) => setError(err.message || 'Could not archive provider.'),
  });

  const activeFields = CREDENTIAL_FIELDS[providerType];

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const body: Record<string, unknown> = {
      provider: providerType,
      name: name.trim() || undefined,
    };
    for (const f of activeFields) {
      const v = fields[f];
      if (v && (typeof v === 'string' ? v.trim() : true)) {
        if (f === 'bank_details' && typeof v === 'string') {
          try { body[f] = JSON.parse(v); }
          catch { setError('bank_details must be valid JSON.'); return; }
        } else {
          body[f] = v;
        }
      }
    }
    createMut.mutate(body as unknown as UpsertProviderRequest);
  }

  const liveProviders = list.data?.filter((p) => !p.archived) ?? [];

  return (
    <div>
      <h1 style={{ display: 'flex', alignItems: 'center', gap: 8, margin: '0 0 16px' }}>
        <CreditCard size={22} /> Payment providers
      </h1>
      <p style={{ color: 'var(--color-text-secondary, #64748b)', marginTop: 0 }}>
        Credentials persist encrypted at rest (AES-256-GCM via the JWT secret).
        The backend never echoes plaintext — existing rows show only which
        fields are populated.
      </p>

      <div style={{ marginBottom: 16 }}>
        {!adding && (
          <button className="btn btn--primary" onClick={() => { setAdding(true); setError(null); }}>
            <Plus size={14} /> Add provider
          </button>
        )}
        {adding && (
          <form onSubmit={submit} style={{ background: 'var(--color-bg-elevated, #f8fafc)', border: '1px solid var(--color-border, #e5e7eb)', borderRadius: 8, padding: 16, maxWidth: 640 }}>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
              <label style={{ fontSize: 12, display: 'flex', flexDirection: 'column' }}>
                <span style={{ marginBottom: 4 }}>Provider type</span>
                <select
                  value={providerType}
                  onChange={(e) => { setProviderType(e.target.value as PaymentProviderType); setFields({}); }}
                  style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }}
                >
                  {PROVIDER_OPTIONS.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
                </select>
              </label>
              <label style={{ fontSize: 12, display: 'flex', flexDirection: 'column' }}>
                <span style={{ marginBottom: 4 }}>Display name (optional)</span>
                <input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Paystack — Live" style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }} />
              </label>
            </div>
            {activeFields.map((f) => (
              <label key={String(f)} style={{ display: 'flex', flexDirection: 'column', fontSize: 12, marginBottom: 8 }}>
                <span style={{ marginBottom: 4 }}>{FIELD_LABELS[f] ?? String(f)}</span>
                {f === 'bank_details' ? (
                  <textarea
                    rows={3}
                    value={(fields[f] as unknown as string) ?? ''}
                    onChange={(e) => setFields({ ...fields, [f]: e.target.value as never })}
                    placeholder='{"bank":"GCB","account":"1234567890"}'
                    style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)', fontFamily: 'monospace' }}
                  />
                ) : (
                  <input
                    type={SECRET_FIELDS.has(f) ? 'password' : 'text'}
                    value={(fields[f] as unknown as string) ?? ''}
                    onChange={(e) => setFields({ ...fields, [f]: e.target.value as never })}
                    autoComplete="new-password"
                    style={{ padding: '6px 8px', borderRadius: 6, border: '1px solid var(--color-border, #e5e7eb)' }}
                  />
                )}
              </label>
            ))}
            {error && <div role="alert" style={{ color: 'var(--color-danger, #dc2626)', fontSize: 13, marginTop: 8 }}>{error}</div>}
            <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
              <button type="submit" className="btn btn--primary" disabled={createMut.isPending}>{createMut.isPending ? 'Saving…' : 'Save provider'}</button>
              <button type="button" className="btn btn--ghost" onClick={() => { setAdding(false); setFields({}); setError(null); }} disabled={createMut.isPending}>Cancel</button>
            </div>
          </form>
        )}
      </div>

      {list.isLoading && <p>Loading providers…</p>}
      {list.isError && <p style={{ color: 'var(--color-danger, #dc2626)' }}>{(list.error as Error)?.message}</p>}
      {list.data && (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr style={{ background: 'var(--color-bg-elevated, #f8fafc)' }}>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Provider</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Name</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Env</th>
              <th style={{ textAlign: 'left', padding: '8px 12px', whiteSpace: 'nowrap' }}>Secret</th>
              <th style={{ textAlign: 'left', padding: '8px 12px', whiteSpace: 'nowrap' }}>Webhook</th>
              <th style={{ textAlign: 'left', padding: '8px 12px', whiteSpace: 'nowrap' }}>Merchant</th>
              <th style={{ textAlign: 'left', padding: '8px 12px' }}>Active</th>
              <th style={{ textAlign: 'right', padding: '8px 12px' }}></th>
            </tr>
          </thead>
          <tbody>
            {liveProviders.length === 0 && (
              <tr><td colSpan={8} style={{ padding: 24, textAlign: 'center', color: 'var(--color-text-secondary, #64748b)' }}>No active payment providers configured.</td></tr>
            )}
            {liveProviders.map((p) => (
              <tr key={p.id} style={{ borderTop: '1px solid var(--color-border, #e5e7eb)' }}>
                <td style={{ padding: '8px 12px', fontFamily: 'monospace' }}>{p.provider}</td>
                <td style={{ padding: '8px 12px' }}>{p.name ?? '—'}</td>
                <td style={{ padding: '8px 12px' }}>{p.environment ?? '—'}</td>
                <td style={{ padding: '8px 12px' }}>{hasMark(p.has_secret_key)}</td>
                <td style={{ padding: '8px 12px' }}>{hasMark(p.has_webhook_secret)}</td>
                <td style={{ padding: '8px 12px' }}>{hasMark(p.has_merchant_id)}</td>
                <td style={{ padding: '8px 12px' }}>{p.enabled ? 'Yes' : 'No'}</td>
                <td style={{ padding: '8px 12px', textAlign: 'right' }}>
                  <button
                    className="btn btn--icon btn--danger"
                    onClick={() => {
                      if (confirm(`Archive ${p.provider}${p.name ? ` (${p.name})` : ''}?`)) {
                        archiveMut.mutate(p.id);
                      }
                    }}
                    disabled={archiveMut.isPending}
                    title="Archive provider"
                    aria-label={`Archive ${p.provider}`}
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
