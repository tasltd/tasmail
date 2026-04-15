// Added: Custom hostname management UI for per-tenant SNI configuration (TMAIL-112)
// PURPOSE: Admin UI for managing custom SMTP/IMAP hostnames per domain
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Globe, CheckCircle, XCircle, ShieldCheck } from 'lucide-react';
import {
  listHostnames,
  createHostname,
  deleteHostname,
  verifyHostname,
} from '../../api/custom-hostnames';
import type { CustomHostname } from '../../api/custom-hostnames';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Domain interface for the domain dropdown
interface Domain {
  id: string;
  name: string;
  active: boolean;
}

export function HostnameManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);

  // Added: Form state for creating new hostname configs
  const [formDomainId, setFormDomainId] = useState('');
  const [formSmtpHostname, setFormSmtpHostname] = useState('');
  const [formImapHostname, setFormImapHostname] = useState('');
  const [formWebmailHostname, setFormWebmailHostname] = useState('');
  const [formAutodiscoverHostname, setFormAutodiscoverHostname] = useState('');

  const { data: hostnames, isLoading } = useQuery({
    queryKey: ['custom-hostnames'],
    queryFn: listHostnames,
  });

  // Added: Fetch domains for the domain dropdown
  const { data: domains } = useQuery<Domain[]>({
    queryKey: ['admin-domains'],
    queryFn: async () => {
      const { apiClient } = await import('../../api/client');
      return apiClient.get<Domain[]>('/admin/domains');
    },
  });

  const createMut = useMutation({
    mutationFn: createHostname,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['custom-hostnames'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      setFormDomainId('');
      setFormSmtpHostname('');
      setFormImapHostname('');
      setFormWebmailHostname('');
      setFormAutodiscoverHostname('');
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteHostname,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['custom-hostnames'] }),
  });

  const verifyMut = useMutation({
    mutationFn: verifyHostname,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['custom-hostnames'] }),
  });

  const handleCreate = (formEvent: React.FormEvent) => {
    formEvent.preventDefault();
    createMut.mutate({
      domain_id: formDomainId,
      smtp_hostname: formSmtpHostname,
      imap_hostname: formImapHostname,
      webmail_hostname: formWebmailHostname || undefined,
      autodiscover_hostname: formAutodiscoverHostname || undefined,
    });
  };

  // Added: Helper to find domain name by ID for display
  const getDomainName = (domainId: string): string => {
    const domain = domains?.find((domainItem: Domain) => domainItem.id === domainId);
    return domain?.name ?? domainId;
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="hostname-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Custom Hostnames</h2>
        <button
          className="btn btn--primary"
          onClick={() => setIsCreating(true)}
        >
          <Plus size={16} /> Add Hostname
        </button>
      </div>

      {/* Added: Create hostname form */}
      {isCreating && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>New Custom Hostname</h3>
          <form onSubmit={handleCreate}>
            <div className="composer__field">
              <label>Domain</label>
              <select
                value={formDomainId}
                onChange={(selectEvent) => setFormDomainId(selectEvent.target.value)}
                required
                data-testid="domain-select"
              >
                <option value="">Select a domain...</option>
                {domains?.map((domainItem: Domain) => (
                  <option key={domainItem.id} value={domainItem.id}>
                    {domainItem.name}
                  </option>
                ))}
              </select>
            </div>
            <div className="composer__field">
              <label>SMTP Hostname</label>
              <input
                value={formSmtpHostname}
                onChange={(inputEvent) => setFormSmtpHostname(inputEvent.target.value)}
                placeholder="smtp.example.com"
                required
              />
            </div>
            <div className="composer__field">
              <label>IMAP Hostname</label>
              <input
                value={formImapHostname}
                onChange={(inputEvent) => setFormImapHostname(inputEvent.target.value)}
                placeholder="imap.example.com"
                required
              />
            </div>
            <div className="composer__field">
              <label>Webmail Hostname (optional)</label>
              <input
                value={formWebmailHostname}
                onChange={(inputEvent) => setFormWebmailHostname(inputEvent.target.value)}
                placeholder="mail.example.com"
              />
            </div>
            <div className="composer__field">
              <label>Autodiscover Hostname (optional)</label>
              <input
                value={formAutodiscoverHostname}
                onChange={(inputEvent) => setFormAutodiscoverHostname(inputEvent.target.value)}
                placeholder="autodiscover.example.com"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" disabled={!formDomainId}>
                Create
              </button>
              <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Hostname list */}
      <div style={{ marginTop: '16px' }}>
        {(!hostnames || hostnames.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No custom hostnames configured. Add one to enable custom SMTP/IMAP domains.
          </p>
        )}
        {hostnames?.map((hostname: CustomHostname) => (
          <div
            key={hostname.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <Globe size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <strong style={{ fontSize: '14px' }}>{getDomainName(hostname.domain_id)}</strong>
                  {/* Added: Verification status badge */}
                  <span
                    data-testid={`status-${hostname.id}`}
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '3px',
                      background: hostname.verified ? 'green' : 'orange',
                      color: 'white',
                    }}
                  >
                    {hostname.verified ? (
                      <><CheckCircle size={10} /> Verified</>
                    ) : (
                      <><XCircle size={10} /> Unverified</>
                    )}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  SMTP: {hostname.smtp_hostname} &middot; IMAP: {hostname.imap_hostname}
                  {hostname.webmail_hostname && <> &middot; Webmail: {hostname.webmail_hostname}</>}
                </div>
                {/* Added: Show DNS verification token when unverified */}
                {!hostname.verified && hostname.dns_verification_token && (
                  <div
                    style={{
                      fontSize: '11px',
                      color: 'var(--color-text-secondary)',
                      marginTop: '4px',
                      fontFamily: 'monospace',
                      background: 'var(--color-surface)',
                      padding: '4px 8px',
                      borderRadius: '4px',
                    }}
                    data-testid={`dns-token-${hostname.id}`}
                  >
                    DNS TXT record: tasmail-verify={hostname.dns_verification_token}
                  </div>
                )}
              </div>
              {/* Added: Verify button — only shown when not yet verified */}
              {!hostname.verified && (
                <button
                  className="btn btn--icon"
                  onClick={() => verifyMut.mutate(hostname.id)}
                  title="Verify DNS"
                  data-testid={`verify-${hostname.id}`}
                >
                  <ShieldCheck size={18} />
                </button>
              )}
              {/* Added: Delete button */}
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(hostname.id)}
                title="Delete"
              >
                <Trash2 size={16} />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
