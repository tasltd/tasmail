// Added: DANE (DNS-based Authentication of Named Entities) management UI for TMAIL-125
// PURPOSE: Allows admins to manage DANE policies and users to view DANE verification status
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Search, ShieldCheck, ShieldAlert, ShieldOff } from 'lucide-react';
import {
  listDanePolicies,
  createDanePolicy,
  deleteDanePolicy,
  lookupTlsa,
  listDaneVerifications,
} from '../../api/dane';
import type { DanePolicy, DaneVerification, DaneResult } from '../../api/dane';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Status badge colors for DANE verification results
const STATUS_COLORS: Record<string, { bg: string; text: string }> = {
  verified: { bg: '#22c55e', text: 'white' },
  failed: { bg: '#ef4444', text: 'white' },
  no_tlsa: { bg: '#f59e0b', text: 'white' },
  disabled: { bg: '#6b7280', text: 'white' },
};

// Added: Status icon component for verification results
function StatusIcon({ status }: { status: string }) {
  if (status === 'verified') return <ShieldCheck size={16} style={{ color: '#22c55e' }} />;
  if (status === 'failed') return <ShieldAlert size={16} style={{ color: '#ef4444' }} />;
  return <ShieldOff size={16} style={{ color: '#6b7280' }} />;
}

export function DaneManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [activeTab, setActiveTab] = useState<'policies' | 'verifications'>('policies');
  const [isCreating, setIsCreating] = useState(false);
  const [formDomain, setFormDomain] = useState('');
  const [formEnforce, setFormEnforce] = useState(false);

  // Added: TLSA lookup tool state
  const [lookupDomain, setLookupDomain] = useState('');
  const [lookupPort, setLookupPort] = useState('');
  const [lookupResult, setLookupResult] = useState<DaneResult | null>(null);

  // Added: Fetch DANE policies (admin)
  const { data: policies, isLoading: policiesLoading } = useQuery({
    queryKey: ['dane-policies'],
    queryFn: listDanePolicies,
    enabled: activeTab === 'policies',
  });

  // Added: Fetch DANE verifications (user)
  const { data: verifications, isLoading: verificationsLoading } = useQuery({
    queryKey: ['dane-verifications'],
    queryFn: () => listDaneVerifications(),
    enabled: activeTab === 'verifications',
  });

  const createMut = useMutation({
    mutationFn: createDanePolicy,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dane-policies'] });
      setIsCreating(false);
      setFormDomain('');
      setFormEnforce(false);
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteDanePolicy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['dane-policies'] }),
  });

  const lookupMut = useMutation({
    mutationFn: lookupTlsa,
    onSuccess: (result) => setLookupResult(result),
  });

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      domain: formDomain,
      enforce: formEnforce || undefined,
    });
  };

  const handleLookup = (e: FormEvent) => {
    e.preventDefault();
    setLookupResult(null);
    lookupMut.mutate({
      domain: lookupDomain,
      port: lookupPort ? parseInt(lookupPort, 10) : undefined,
    });
  };

  return (
    <div className="dane-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>DANE / TLSA</h2>
      </div>

      {/* Added: Tab navigation */}
      <div style={{ display: 'flex', gap: '8px', marginTop: '12px', borderBottom: '1px solid var(--color-border)', paddingBottom: '8px' }}>
        <button
          className={`btn ${activeTab === 'policies' ? 'btn--primary' : ''}`}
          onClick={() => setActiveTab('policies')}
        >
          Policies
        </button>
        <button
          className={`btn ${activeTab === 'verifications' ? 'btn--primary' : ''}`}
          onClick={() => setActiveTab('verifications')}
        >
          Verifications
        </button>
      </div>

      {/* Added: Policies tab */}
      {activeTab === 'policies' && (
        <div style={{ marginTop: '16px' }}>
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginBottom: '12px' }}>
            <button className="btn btn--primary" onClick={() => setIsCreating(true)}>
              <Plus size={16} /> Add Policy
            </button>
          </div>

          {/* Added: Create policy form */}
          {isCreating && (
            <div style={{ marginBottom: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
              <h3 style={{ marginBottom: '12px' }}>New DANE Policy</h3>
              <form onSubmit={handleCreate}>
                <div className="composer__field">
                  <label>Domain</label>
                  <input
                    value={formDomain}
                    onChange={(e) => setFormDomain(e.target.value)}
                    placeholder="example.com"
                    required
                  />
                </div>
                <div className="composer__field" style={{ flexDirection: 'row', alignItems: 'center', gap: '8px' }}>
                  <input
                    type="checkbox"
                    id="dane-enforce"
                    checked={formEnforce}
                    onChange={(e) => setFormEnforce(e.target.checked)}
                  />
                  <label htmlFor="dane-enforce">Enforce (reject delivery if DANE fails)</label>
                </div>
                <div className="composer__actions">
                  <button type="submit" className="btn btn--primary" disabled={!formDomain.trim()}>
                    Create
                  </button>
                  <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                    Cancel
                  </button>
                </div>
              </form>
            </div>
          )}

          {/* Added: TLSA Lookup Tool */}
          <div style={{ marginBottom: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
            <h3 style={{ marginBottom: '12px' }}>
              <Search size={16} style={{ marginRight: '6px', verticalAlign: 'text-bottom' }} />
              TLSA Lookup
            </h3>
            <form onSubmit={handleLookup} style={{ display: 'flex', gap: '8px', alignItems: 'flex-end' }}>
              <div className="composer__field" style={{ flex: 1 }}>
                <label>Domain</label>
                <input
                  value={lookupDomain}
                  onChange={(e) => setLookupDomain(e.target.value)}
                  placeholder="mail.example.com"
                  required
                />
              </div>
              <div className="composer__field" style={{ width: '100px' }}>
                <label>Port</label>
                <input
                  value={lookupPort}
                  onChange={(e) => setLookupPort(e.target.value)}
                  placeholder="25"
                  type="number"
                />
              </div>
              <button type="submit" className="btn btn--primary" disabled={!lookupDomain.trim() || lookupMut.isPending}>
                Lookup
              </button>
            </form>
            {lookupResult && (
              <div style={{ marginTop: '12px', padding: '12px', background: 'var(--color-bg-secondary)', borderRadius: '6px', fontSize: '13px' }} data-testid="lookup-result">
                <div><strong>Status:</strong> {lookupResult.status}</div>
                <div><strong>Message:</strong> {lookupResult.message}</div>
                {lookupResult.tlsa_records.length > 0 && (
                  <div style={{ marginTop: '8px' }}>
                    <strong>TLSA Records:</strong>
                    {lookupResult.tlsa_records.map((r, i) => (
                      <div key={i} style={{ fontFamily: 'monospace', fontSize: '12px', marginTop: '4px' }}>
                        {r.usage} {r.selector} {r.matching_type} {r.cert_data}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>

          {/* Added: Policy list */}
          {policiesLoading && <LoadingSkeleton rows={3} />}
          {!policiesLoading && (!policies || policies.length === 0) && !isCreating && (
            <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
              No DANE policies configured. Add one to enable DANE verification for a domain.
            </p>
          )}
          {policies?.map((policy: DanePolicy) => (
            <div
              key={policy.id}
              style={{ padding: '12px', borderBottom: '1px solid var(--color-border)', display: 'flex', alignItems: 'center', gap: '12px' }}
            >
              <ShieldCheck size={18} style={{ color: policy.enforce ? '#22c55e' : 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <strong style={{ fontSize: '14px' }}>{policy.domain}</strong>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: policy.enforce ? '#22c55e' : '#6b7280',
                      color: 'white',
                    }}
                  >
                    {policy.enforce ? 'Enforcing' : 'Monitor'}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {policy.tlsa_records.length} TLSA record{policy.tlsa_records.length !== 1 ? 's' : ''}
                  {policy.last_checked_at && (
                    <> &middot; Last checked {new Date(policy.last_checked_at).toLocaleDateString()}</>
                  )}
                </div>
              </div>
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(policy.id)}
                title="Delete"
              >
                <Trash2 size={16} />
              </button>
            </div>
          ))}
        </div>
      )}

      {/* Added: Verifications tab */}
      {activeTab === 'verifications' && (
        <div style={{ marginTop: '16px' }}>
          {verificationsLoading && <LoadingSkeleton rows={4} />}
          {!verificationsLoading && (!verifications || verifications.length === 0) && (
            <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
              No DANE verifications yet. Verifications will appear after sending emails to DANE-enabled domains.
            </p>
          )}
          {verifications && verifications.length > 0 && (
            <table style={{ width: '100%', fontSize: '13px', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ borderBottom: '2px solid var(--color-border)' }}>
                  <th style={{ textAlign: 'left', padding: '8px' }}>Domain</th>
                  <th style={{ textAlign: 'left', padding: '8px' }}>Message ID</th>
                  <th style={{ textAlign: 'left', padding: '8px' }}>Status</th>
                  <th style={{ textAlign: 'left', padding: '8px' }}>Checked</th>
                </tr>
              </thead>
              <tbody>
                {verifications.map((v: DaneVerification) => {
                  const colors = STATUS_COLORS[v.dane_status] || STATUS_COLORS.disabled;
                  return (
                    <tr key={v.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                      <td style={{ padding: '8px' }}>{v.recipient_domain}</td>
                      <td style={{ padding: '8px', fontFamily: 'monospace', fontSize: '11px', maxWidth: '200px', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                        {v.message_id}
                      </td>
                      <td style={{ padding: '8px' }}>
                        <span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}>
                          <StatusIcon status={v.dane_status} />
                          <span
                            style={{
                              fontSize: '11px',
                              padding: '1px 6px',
                              borderRadius: '10px',
                              background: colors.bg,
                              color: colors.text,
                            }}
                          >
                            {v.dane_status}
                          </span>
                        </span>
                      </td>
                      <td style={{ padding: '8px' }}>
                        {new Date(v.checked_at).toLocaleString()}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </div>
      )}
    </div>
  );
}
