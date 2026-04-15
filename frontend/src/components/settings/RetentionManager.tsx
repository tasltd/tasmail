// Added: Retention policy and legal hold management UI for TMAIL-109
// PURPOSE: Allows admins to manage email retention policies and legal holds
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Archive, Shield, Unlock } from 'lucide-react';
import {
  listRetentionPolicies,
  createRetentionPolicy,
  updateRetentionPolicy,
  deleteRetentionPolicy,
  listLegalHolds,
  createLegalHold,
  releaseLegalHold,
} from '../../api/retention';
import type { RetentionPolicy, LegalHold } from '../../api/retention';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

export function RetentionManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Form visibility state
  const [showPolicyForm, setShowPolicyForm] = useState(false);
  const [showHoldForm, setShowHoldForm] = useState(false);

  // Added: Policy form state
  const [policyName, setPolicyName] = useState('');
  const [policyDescription, setPolicyDescription] = useState('');
  const [policyRetentionDays, setPolicyRetentionDays] = useState('');
  const [policyFolderPattern, setPolicyFolderPattern] = useState('');
  const [policyApplyToAll, setPolicyApplyToAll] = useState(false);

  // Added: Legal hold form state
  const [holdUserId, setHoldUserId] = useState('');
  const [holdReason, setHoldReason] = useState('');

  // Added: Fetch retention policies
  const { data: policies, isLoading: policiesLoading } = useQuery({
    queryKey: ['retention-policies'],
    queryFn: listRetentionPolicies,
  });

  // Added: Fetch legal holds
  const { data: holds, isLoading: holdsLoading } = useQuery({
    queryKey: ['legal-holds'],
    queryFn: listLegalHolds,
  });

  // Added: Create retention policy mutation
  const createPolicyMut = useMutation({
    mutationFn: createRetentionPolicy,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['retention-policies'] });
      setShowPolicyForm(false);
      setPolicyName('');
      setPolicyDescription('');
      setPolicyRetentionDays('');
      setPolicyFolderPattern('');
      setPolicyApplyToAll(false);
    },
  });

  // Added: Update retention policy mutation (for apply_to_all toggle)
  const updatePolicyMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Parameters<typeof updateRetentionPolicy>[1] }) =>
      updateRetentionPolicy(id, data),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['retention-policies'] }),
  });

  // Added: Delete retention policy mutation
  const deletePolicyMut = useMutation({
    mutationFn: deleteRetentionPolicy,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['retention-policies'] }),
  });

  // Added: Create legal hold mutation
  const createHoldMut = useMutation({
    mutationFn: createLegalHold,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['legal-holds'] });
      setShowHoldForm(false);
      setHoldUserId('');
      setHoldReason('');
    },
  });

  // Added: Release legal hold mutation
  const releaseHoldMut = useMutation({
    mutationFn: releaseLegalHold,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['legal-holds'] }),
  });

  const handleCreatePolicy = (e: React.FormEvent) => {
    e.preventDefault();
    createPolicyMut.mutate({
      name: policyName,
      description: policyDescription || undefined,
      retention_days: parseInt(policyRetentionDays, 10),
      folder_pattern: policyFolderPattern || undefined,
      apply_to_all: policyApplyToAll,
    });
  };

  const handleCreateHold = (e: React.FormEvent) => {
    e.preventDefault();
    createHoldMut.mutate({
      user_id: holdUserId,
      reason: holdReason,
    });
  };

  if (policiesLoading || holdsLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="retention-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Retention & Legal Hold</h2>
      </div>

      {/* Added: Retention Policies section */}
      <section style={{ marginTop: '24px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
          <h3 style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Archive size={18} />
            Retention Policies
          </h3>
          <button
            className="btn btn--primary"
            onClick={() => setShowPolicyForm(true)}
          >
            <Plus size={16} /> Add Policy
          </button>
        </div>

        {/* Added: Create policy form */}
        {showPolicyForm && (
          <div
            style={{
              padding: '16px',
              border: '1px solid var(--color-border)',
              borderRadius: '8px',
              marginBottom: '16px',
            }}
          >
            <h4 style={{ marginBottom: '12px' }}>New Retention Policy</h4>
            <form onSubmit={handleCreatePolicy}>
              <div className="composer__field">
                <label>Name</label>
                <input
                  value={policyName}
                  onChange={(e) => setPolicyName(e.target.value)}
                  placeholder="e.g., Trash Cleanup"
                  required
                />
              </div>
              <div className="composer__field">
                <label>Description</label>
                <input
                  value={policyDescription}
                  onChange={(e) => setPolicyDescription(e.target.value)}
                  placeholder="Optional description"
                />
              </div>
              <div className="composer__field">
                <label>Retention Days</label>
                <input
                  type="number"
                  min="1"
                  value={policyRetentionDays}
                  onChange={(e) => setPolicyRetentionDays(e.target.value)}
                  placeholder="e.g., 30"
                  required
                />
              </div>
              <div className="composer__field">
                <label>Folder Pattern</label>
                <input
                  value={policyFolderPattern}
                  onChange={(e) => setPolicyFolderPattern(e.target.value)}
                  placeholder="e.g., Trash, Spam (leave empty for all)"
                />
              </div>
              <div className="composer__field">
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <input
                    type="checkbox"
                    checked={policyApplyToAll}
                    onChange={(e) => setPolicyApplyToAll(e.target.checked)}
                  />
                  Apply to all users
                </label>
              </div>
              <div className="composer__actions">
                <button type="submit" className="btn btn--primary" disabled={!policyName || !policyRetentionDays}>
                  Create
                </button>
                <button type="button" className="btn" onClick={() => setShowPolicyForm(false)}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        )}

        {/* Added: Policy list */}
        {(!policies || policies.length === 0) && !showPolicyForm && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No retention policies configured. Add one to set auto-deletion rules.
          </p>
        )}
        {policies?.map((policy: RetentionPolicy) => (
          <div
            key={policy.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
          >
            <Archive size={18} style={{ color: 'var(--color-text-secondary)' }} />
            <div style={{ flex: 1 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <strong style={{ fontSize: '14px' }}>{policy.name}</strong>
                <span
                  style={{
                    fontSize: '11px',
                    padding: '1px 6px',
                    borderRadius: '10px',
                    background: policy.apply_to_all ? 'var(--color-primary, #3b82f6)' : 'gray',
                    color: 'white',
                  }}
                >
                  {policy.apply_to_all ? 'Global' : 'Selective'}
                </span>
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                {policy.retention_days} days
                {policy.folder_pattern && <> &middot; Folder: {policy.folder_pattern}</>}
                {policy.description && <> &middot; {policy.description}</>}
              </div>
            </div>
            {/* Added: Toggle apply_to_all */}
            <button
              className="btn btn--icon"
              onClick={() =>
                updatePolicyMut.mutate({ id: policy.id, data: { apply_to_all: !policy.apply_to_all } })
              }
              title={policy.apply_to_all ? 'Make selective' : 'Apply to all'}
              data-testid={`toggle-policy-${policy.id}`}
            >
              {policy.apply_to_all ? 'Global' : 'Selective'}
            </button>
            <button
              className="btn btn--icon btn--danger"
              onClick={() => deletePolicyMut.mutate(policy.id)}
              title="Delete policy"
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </section>

      {/* Added: Legal Holds section */}
      <section style={{ marginTop: '32px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
          <h3 style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Shield size={18} />
            Legal Holds
          </h3>
          <button
            className="btn btn--primary"
            onClick={() => setShowHoldForm(true)}
          >
            <Plus size={16} /> Place Hold
          </button>
        </div>

        {/* Added: Place hold form */}
        {showHoldForm && (
          <div
            style={{
              padding: '16px',
              border: '1px solid var(--color-border)',
              borderRadius: '8px',
              marginBottom: '16px',
            }}
          >
            <h4 style={{ marginBottom: '12px' }}>Place Legal Hold</h4>
            <form onSubmit={handleCreateHold}>
              <div className="composer__field">
                <label>User ID</label>
                <input
                  value={holdUserId}
                  onChange={(e) => setHoldUserId(e.target.value)}
                  placeholder="User UUID"
                  required
                />
              </div>
              <div className="composer__field">
                <label>Reason</label>
                <input
                  value={holdReason}
                  onChange={(e) => setHoldReason(e.target.value)}
                  placeholder="e.g., Ongoing litigation, court order #12345"
                  required
                />
              </div>
              <div className="composer__actions">
                <button type="submit" className="btn btn--primary" disabled={!holdUserId || !holdReason}>
                  Place Hold
                </button>
                <button type="button" className="btn" onClick={() => setShowHoldForm(false)}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        )}

        {/* Added: Legal holds list */}
        {(!holds || holds.length === 0) && !showHoldForm && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No legal holds active. Place a hold to prevent email deletion for a user.
          </p>
        )}
        {holds?.map((hold: LegalHold) => (
          <div
            key={hold.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
          >
            <Shield size={18} style={{ color: hold.active ? 'orange' : 'var(--color-text-secondary)' }} />
            <div style={{ flex: 1 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <strong style={{ fontSize: '14px' }}>User: {hold.user_id.slice(0, 8)}...</strong>
                <span
                  style={{
                    fontSize: '11px',
                    padding: '1px 6px',
                    borderRadius: '10px',
                    background: hold.active ? 'orange' : 'gray',
                    color: 'white',
                  }}
                >
                  {hold.active ? 'Active' : 'Released'}
                </span>
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                {hold.reason}
                {' '}&middot; Placed {new Date(hold.created_at).toLocaleDateString()}
                {hold.released_at && <> &middot; Released {new Date(hold.released_at).toLocaleDateString()}</>}
              </div>
            </div>
            {hold.active && (
              <button
                className="btn btn--icon"
                onClick={() => releaseHoldMut.mutate(hold.id)}
                title="Release hold"
                data-testid={`release-${hold.id}`}
              >
                <Unlock size={16} />
              </button>
            )}
          </div>
        ))}
      </section>
    </div>
  );
}
