/**
 * PURPOSE: Settings UI for managing shared mailbox ACLs
 * CONSTRAINTS: Only users with can_admin permission on a mailbox can manage its ACL
 * EXTERNAL: Uses sharedMailboxApi for all CRUD operations against /shared-mailboxes endpoints
 */
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Mailbox, Trash2, UserPlus, ChevronDown, ChevronRight, Shield } from 'lucide-react';
import { sharedMailboxApi } from '../../api/shared-mailboxes';
import type { SharedMailboxView, SharedMailboxAclWithUser, GrantAccessRequest } from '../../types/shared-mailboxes';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Permission badge labels for display
const PERMISSION_LABELS = [
  { key: 'can_read', label: 'Read' },
  { key: 'can_write', label: 'Write' },
  { key: 'can_delete', label: 'Delete' },
  { key: 'can_admin', label: 'Admin' },
] as const;

// Added: Grant access form for adding ACL entries to a shared mailbox
function GrantAccessForm({
  mailboxId,
  onSuccess,
}: {
  mailboxId: string;
  onSuccess: () => void;
}) {
  const queryClient = useQueryClient();
  const [grantedTo, setGrantedTo] = useState('');
  const [permissions, setPermissions] = useState<GrantAccessRequest>({
    granted_to: '',
    can_read: true,
    can_write: false,
    can_delete: false,
    can_admin: false,
  });

  const grantMutation = useMutation({
    mutationFn: (data: GrantAccessRequest) => sharedMailboxApi.grantAccess(mailboxId, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['shared-mailbox-acl', mailboxId] });
      // Added: Reset form state after successful grant
      setGrantedTo('');
      setPermissions({ granted_to: '', can_read: true, can_write: false, can_delete: false, can_admin: false });
      onSuccess();
    },
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!grantedTo.trim()) return;
    grantMutation.mutate({
      granted_to: grantedTo.trim(),
      can_read: permissions.can_read,
      can_write: permissions.can_write,
      can_delete: permissions.can_delete,
      can_admin: permissions.can_admin,
    });
  };

  return (
    <form className="settings-form" onSubmit={handleSubmit} style={{ marginTop: '12px' }}>
      <div className="form-group">
        <label>User ID</label>
        <input
          type="text"
          value={grantedTo}
          onChange={(e) => setGrantedTo(e.target.value)}
          placeholder="User UUID to grant access"
          required
        />
      </div>
      <div className="form-group">
        <label>Permissions</label>
        <div style={{ display: 'flex', gap: '16px', flexWrap: 'wrap' }}>
          {PERMISSION_LABELS.map(({ key, label }) => (
            <label key={key} style={{ display: 'flex', alignItems: 'center', gap: '4px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={!!permissions[key]}
                onChange={(e) => setPermissions({ ...permissions, [key]: e.target.checked })}
              />
              {label}
            </label>
          ))}
        </div>
      </div>
      <div className="form-actions">
        <button type="submit" className="btn btn--primary" disabled={grantMutation.isPending}>
          {grantMutation.isPending ? 'Granting...' : 'Grant Access'}
        </button>
      </div>
      {grantMutation.isError && (
        <p style={{ color: 'var(--color-error, red)', marginTop: '8px' }}>
          Failed to grant access. Please check the user ID and try again.
        </p>
      )}
    </form>
  );
}

// Added: ACL list item with revoke button for a single shared mailbox
function AclEntry({
  entry,
  mailboxId,
}: {
  entry: SharedMailboxAclWithUser;
  mailboxId: string;
}) {
  const queryClient = useQueryClient();

  const revokeMutation = useMutation({
    mutationFn: () => sharedMailboxApi.revokeAccess(mailboxId, entry.granted_to),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['shared-mailbox-acl', mailboxId] }),
  });

  // Added: Collect active permission labels for display
  const activePermissions = PERMISSION_LABELS
    .filter(({ key }) => entry[key])
    .map(({ label }) => label);

  return (
    <div
      className="acl-entry"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
        padding: '8px 12px',
        borderBottom: '1px solid var(--border)',
      }}
    >
      <div style={{ flex: 1 }}>
        <strong>{entry.granted_to_username}</strong>
        <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>
          {activePermissions.join(', ') || 'No permissions'}
        </div>
      </div>
      <button
        className="btn btn--icon"
        onClick={() => {
          if (confirm(`Revoke access for ${entry.granted_to_username}?`)) {
            revokeMutation.mutate();
          }
        }}
        title="Revoke access"
        disabled={revokeMutation.isPending}
      >
        <Trash2 size={16} />
      </button>
    </div>
  );
}

// Added: Expandable mailbox item showing ACL list when admin clicks
function MailboxItem({
  mailbox,
  expanded,
  onToggle,
}: {
  mailbox: SharedMailboxView;
  expanded: boolean;
  onToggle: () => void;
}) {
  const [showGrantForm, setShowGrantForm] = useState(false);

  // Added: Only fetch ACL when expanded and user has admin permission
  const { data: aclEntries = [], isLoading: aclLoading } = useQuery({
    queryKey: ['shared-mailbox-acl', mailbox.mailbox_id],
    queryFn: () => sharedMailboxApi.listAcl(mailbox.mailbox_id),
    enabled: expanded && mailbox.can_admin,
  });

  // Added: Collect this user's permissions for display
  const myPermissions = PERMISSION_LABELS
    .filter(({ key }) => mailbox[key])
    .map(({ label }) => label);

  return (
    <div className="mailbox-item" style={{ borderBottom: '1px solid var(--border)' }}>
      <div
        className="mailbox-item__header"
        onClick={mailbox.can_admin ? onToggle : undefined}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          padding: '12px',
          cursor: mailbox.can_admin ? 'pointer' : 'default',
        }}
      >
        {mailbox.can_admin ? (
          expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />
        ) : (
          <Mailbox size={16} />
        )}
        <div style={{ flex: 1 }}>
          <strong>{mailbox.display_name || mailbox.username}</strong>
          <span style={{ marginLeft: '8px', fontSize: '0.85em', color: 'var(--text-secondary)' }}>
            {mailbox.username}
          </span>
          <div style={{ fontSize: '0.85em', color: 'var(--text-secondary)' }}>
            {myPermissions.join(', ')}
          </div>
        </div>
        {mailbox.can_admin && (
          <span className="badge" title="You have admin access">
            <Shield size={14} /> Admin
          </span>
        )}
      </div>

      {expanded && mailbox.can_admin && (
        <div style={{ padding: '0 12px 12px 36px' }}>
          {aclLoading ? (
            <LoadingSkeleton rows={3} />
          ) : (
            <>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '8px' }}>
                <h4 style={{ margin: 0 }}>Access Control</h4>
                <button
                  className="btn btn--text"
                  onClick={() => setShowGrantForm(!showGrantForm)}
                >
                  <UserPlus size={14} /> Grant Access
                </button>
              </div>

              {showGrantForm && (
                <GrantAccessForm
                  mailboxId={mailbox.mailbox_id}
                  onSuccess={() => setShowGrantForm(false)}
                />
              )}

              {aclEntries.length === 0 ? (
                <p style={{ color: 'var(--text-secondary)', fontStyle: 'italic' }}>
                  No ACL entries. Grant access to allow other users to use this mailbox.
                </p>
              ) : (
                <div className="acl-list">
                  {aclEntries.map((entry) => (
                    <AclEntry key={entry.id} entry={entry} mailboxId={mailbox.mailbox_id} />
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
}

// Added: Main shared mailbox manager component for TMAIL-96
export function SharedMailboxManager() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [expandedMailbox, setExpandedMailbox] = useState<string | null>(null);

  const { data: mailboxes = [], isLoading, isError } = useQuery({
    queryKey: ['shared-mailboxes'],
    queryFn: sharedMailboxApi.listAccessible,
  });

  if (isLoading) return <LoadingSkeleton rows={5} />;

  return (
    <div className="settings-panel">
      <div className="settings-panel__header">
        <button className="btn btn--text" onClick={() => setViewMode('list')}>
          <ArrowLeft size={16} /> Back
        </button>
        <h2><Mailbox size={20} /> Shared Mailboxes</h2>
      </div>

      {isError && (
        <p style={{ color: 'var(--color-error, red)', padding: '12px' }}>
          Failed to load shared mailboxes. Please try again.
        </p>
      )}

      {!isError && mailboxes.length === 0 ? (
        <p className="empty-state" style={{ textAlign: 'center', color: 'var(--text-secondary)', padding: '40px 0' }}>
          No shared mailboxes available. Contact an administrator to get access.
        </p>
      ) : (
        <div className="shared-mailbox-list">
          {mailboxes.map((mailbox) => (
            <MailboxItem
              key={mailbox.mailbox_id}
              mailbox={mailbox}
              expanded={expandedMailbox === mailbox.mailbox_id}
              onToggle={() =>
                setExpandedMailbox(
                  expandedMailbox === mailbox.mailbox_id ? null : mailbox.mailbox_id
                )
              }
            />
          ))}
        </div>
      )}
    </div>
  );
}
