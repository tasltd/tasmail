// Added: Email delegation management UI for TMAIL-97
import React from 'react';
import { useState } from 'react';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Plus, Trash2, UserCheck, Users } from 'lucide-react';
import {
  listDelegations,
  listGrantedDelegations,
  grantDelegation,
  revokeDelegation,
} from '../../api/delegation';
import type { EmailDelegation, DelegationType } from '../../api/delegation';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Manage email delegations — view received/granted delegations, grant new, revoke
 * EXTERNAL: Uses /api/delegation endpoints via TanStack Query
 * NOTE: Delegations allow one user to send emails as or on behalf of another
 */
export function DelegationManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Tab and form state
  const [activeTab, setActiveTab] = useState<'received' | 'granted'>('received');
  const [showForm, setShowForm] = useState(false);
  const [delegateEmail, setDelegateEmail] = useState('');
  const [delegationType, setDelegationType] = useState<DelegationType>('send_as');

  // Added: Fetch delegations received (others granted to me)
  const { data: receivedDelegations = [], isLoading: loadingReceived } = useQuery({
    queryKey: ['delegations'],
    queryFn: listDelegations,
  });

  // Added: Fetch delegations granted (I granted to others)
  const { data: grantedDelegations = [], isLoading: loadingGranted } = useQuery({
    queryKey: ['delegations-granted'],
    queryFn: listGrantedDelegations,
  });

  const grantMutation = useMutation({
    mutationFn: grantDelegation,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['delegations'] });
      queryClient.invalidateQueries({ queryKey: ['delegations-granted'] });
      setShowForm(false);
      setDelegateEmail('');
      setDelegationType('send_as');
    },
  });

  const revokeMutation = useMutation({
    mutationFn: revokeDelegation,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['delegations'] });
      queryClient.invalidateQueries({ queryKey: ['delegations-granted'] });
    },
  });

  // Added: Submit grant delegation form
  const handleGrant = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    // NOTE: grantor_id and delegate_id are resolved server-side from the email;
    // we pass the email as delegate_id for the API to resolve
    grantMutation.mutate({
      grantor_id: '',
      delegate_id: delegateEmail,
      delegation_type: delegationType,
    });
  };

  /**
   * PURPOSE: Format delegation type for display
   */
  const formatDelegationType = (delegationTypeValue: DelegationType): string => {
    return delegationTypeValue === 'send_as' ? 'Send As' : 'Send on Behalf';
  };

  if (loadingReceived || loadingGranted) return <LoadingSkeleton rows={4} />;

  const activeDelegations =
    activeTab === 'received' ? receivedDelegations : grantedDelegations;

  return (
    <div className="delegation-manager" style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Email Delegation</h2>
        {!showForm && (
          <button
            className="btn btn--primary"
            onClick={() => setShowForm(true)}
          >
            <Plus size={16} /> Grant Delegation
          </button>
        )}
      </div>

      {/* Added: Tab switcher for received vs granted delegations */}
      <div
        style={{
          display: 'flex',
          gap: '0',
          marginTop: '16px',
          borderBottom: '2px solid var(--color-border)',
        }}
      >
        <button
          className="btn btn--text"
          onClick={() => setActiveTab('received')}
          style={{
            padding: '8px 16px',
            borderBottom:
              activeTab === 'received'
                ? '2px solid var(--color-primary, #0066cc)'
                : '2px solid transparent',
            fontWeight: activeTab === 'received' ? 600 : 400,
          }}
        >
          <UserCheck size={16} style={{ marginRight: '6px' }} />
          Received
        </button>
        <button
          className="btn btn--text"
          onClick={() => setActiveTab('granted')}
          style={{
            padding: '8px 16px',
            borderBottom:
              activeTab === 'granted'
                ? '2px solid var(--color-primary, #0066cc)'
                : '2px solid transparent',
            fontWeight: activeTab === 'granted' ? 600 : 400,
          }}
        >
          <Users size={16} style={{ marginRight: '6px' }} />
          Granted
        </button>
      </div>

      {/* Added: Grant delegation form */}
      {showForm && (
        <form
          onSubmit={handleGrant}
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>Grant New Delegation</h3>
          <div className="composer__field">
            <label>Delegate Email</label>
            <input
              type="email"
              value={delegateEmail}
              onChange={(e) => setDelegateEmail(e.target.value)}
              placeholder="user@example.com"
              required
              data-testid="delegate-email"
            />
          </div>
          <div className="composer__field">
            <label>Delegation Type</label>
            <select
              value={delegationType}
              onChange={(e) => setDelegationType(e.target.value as DelegationType)}
              data-testid="delegation-type"
            >
              <option value="send_as">Send As</option>
              <option value="send_on_behalf">Send on Behalf</option>
            </select>
          </div>
          <div className="composer__actions" style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
            <button
              type="submit"
              className="btn btn--primary"
              disabled={grantMutation.isPending}
            >
              {grantMutation.isPending ? 'Granting...' : 'Grant'}
            </button>
            <button type="button" className="btn" onClick={() => setShowForm(false)}>
              Cancel
            </button>
          </div>
        </form>
      )}

      {/* Added: Delegation list */}
      <div style={{ marginTop: '16px' }}>
        {activeDelegations.length === 0 && (
          <p
            style={{
              color: 'var(--color-text-secondary)',
              textAlign: 'center',
              padding: '24px',
            }}
          >
            {activeTab === 'received'
              ? 'No delegations received. Other users can grant you permission to send as them.'
              : 'No delegations granted. Grant delegation to allow others to send as you.'}
          </p>
        )}
        {activeDelegations.map((delegation: EmailDelegation) => (
          <div
            key={delegation.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ flex: 1 }}>
              <strong>
                {activeTab === 'received'
                  ? delegation.grantor_id
                  : delegation.delegate_id}
              </strong>
              <span
                style={{
                  marginLeft: '8px',
                  fontSize: '11px',
                  background: 'var(--color-bg-secondary)',
                  padding: '1px 6px',
                  borderRadius: '10px',
                  border: '1px solid var(--color-border)',
                }}
              >
                {formatDelegationType(delegation.delegation_type)}
              </span>
              <div
                style={{
                  fontSize: '12px',
                  color: 'var(--color-text-secondary)',
                  marginTop: '4px',
                }}
              >
                Granted {new Date(delegation.created_at).toLocaleDateString()}
              </div>
            </div>
            {/* NOTE: Only show revoke for delegations the user has granted */}
            {activeTab === 'granted' && (
              <button
                className="btn btn--icon btn--danger"
                onClick={() => revokeMutation.mutate(delegation.id)}
                title="Revoke"
                data-testid={`revoke-${delegation.id}`}
              >
                <Trash2 size={16} />
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
