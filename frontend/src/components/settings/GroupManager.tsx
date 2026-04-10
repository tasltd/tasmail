import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Users, Plus, Trash2, UserPlus, ChevronDown, ChevronRight } from 'lucide-react';
import { groupsApi } from '../../api/groups';
import type { DistributionGroup } from '../../types/groups';

export function GroupManager() {
  const queryClient = useQueryClient();
  const [expandedGroup, setExpandedGroup] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newGroup, setNewGroup] = useState({ name: '', address: '', description: '' });
  const [newMember, setNewMember] = useState('');

  const { data: groups = [], isLoading } = useQuery({
    queryKey: ['groups'],
    queryFn: groupsApi.list,
  });

  const createMutation = useMutation({
    mutationFn: groupsApi.create,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['groups'] });
      setShowCreate(false);
      setNewGroup({ name: '', address: '', description: '' });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: groupsApi.delete,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['groups'] }),
  });

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newGroup.name || !newGroup.address) return;
    // NOTE: domain_id needs to come from user's domain — using placeholder
    createMutation.mutate({
      name: newGroup.name,
      address: newGroup.address,
      description: newGroup.description || undefined,
      domain_id: '', // Added: Will be resolved from user's primary domain
    });
  };

  if (isLoading) return <div className="loading">Loading groups...</div>;

  return (
    <div className="settings-panel">
      <div className="settings-panel__header">
        <h2><Users size={20} /> Distribution Groups</h2>
        <button className="btn btn--primary" onClick={() => setShowCreate(!showCreate)}>
          <Plus size={16} /> New Group
        </button>
      </div>

      {showCreate && (
        <form className="settings-form" onSubmit={handleCreate}>
          <div className="form-group">
            <label>Group Name</label>
            <input
              type="text"
              value={newGroup.name}
              onChange={(e) => setNewGroup({ ...newGroup, name: e.target.value })}
              placeholder="Engineering Team"
              required
            />
          </div>
          <div className="form-group">
            <label>Group Address</label>
            <input
              type="email"
              value={newGroup.address}
              onChange={(e) => setNewGroup({ ...newGroup, address: e.target.value })}
              placeholder="engineering@example.com"
              required
            />
          </div>
          <div className="form-group">
            <label>Description</label>
            <input
              type="text"
              value={newGroup.description}
              onChange={(e) => setNewGroup({ ...newGroup, description: e.target.value })}
              placeholder="Optional description"
            />
          </div>
          <div className="form-actions">
            <button type="submit" className="btn btn--primary" disabled={createMutation.isPending}>
              {createMutation.isPending ? 'Creating...' : 'Create Group'}
            </button>
            <button type="button" className="btn" onClick={() => setShowCreate(false)}>Cancel</button>
          </div>
        </form>
      )}

      {groups.length === 0 && !showCreate && (
        <p className="empty-state">No distribution groups yet. Create one to get started.</p>
      )}

      <div className="groups-list">
        {groups.map((group) => (
          <GroupItem
            key={group.id}
            group={group}
            expanded={expandedGroup === group.id}
            onToggle={() => setExpandedGroup(expandedGroup === group.id ? null : group.id)}
            onDelete={() => {
              if (confirm(`Delete group "${group.name}"?`)) {
                deleteMutation.mutate(group.id);
              }
            }}
            newMember={expandedGroup === group.id ? newMember : ''}
            onNewMemberChange={setNewMember}
          />
        ))}
      </div>
    </div>
  );
}

// Added: Individual group item with expandable member list
function GroupItem({
  group,
  expanded,
  onToggle,
  onDelete,
  newMember,
  onNewMemberChange,
}: {
  group: DistributionGroup;
  expanded: boolean;
  onToggle: () => void;
  onDelete: () => void;
  newMember: string;
  onNewMemberChange: (v: string) => void;
}) {
  const queryClient = useQueryClient();

  const { data: members = [] } = useQuery({
    queryKey: ['group-members', group.id],
    queryFn: () => groupsApi.listMembers(group.id),
    enabled: expanded,
  });

  const addMemberMutation = useMutation({
    mutationFn: (address: string) => groupsApi.addMember(group.id, { member_address: address }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['group-members', group.id] });
      onNewMemberChange('');
    },
  });

  const removeMemberMutation = useMutation({
    mutationFn: (address: string) => groupsApi.removeMember(group.id, address),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['group-members', group.id] }),
  });

  const handleAddMember = (e: React.FormEvent) => {
    e.preventDefault();
    if (newMember.includes('@')) {
      addMemberMutation.mutate(newMember);
    }
  };

  return (
    <div className="group-item">
      <div className="group-item__header" onClick={onToggle}>
        {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        <div className="group-item__info">
          <strong>{group.name}</strong>
          <span className="group-item__address">{group.address}</span>
          {group.description && <span className="group-item__desc">{group.description}</span>}
        </div>
        <div className="group-item__actions">
          {!group.active && <span className="badge badge--inactive">Inactive</span>}
          <button className="btn btn--icon" onClick={(e) => { e.stopPropagation(); onDelete(); }}>
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      {expanded && (
        <div className="group-item__members">
          <form className="group-member-form" onSubmit={handleAddMember}>
            <input
              type="email"
              value={newMember}
              onChange={(e) => onNewMemberChange(e.target.value)}
              placeholder="Add member email..."
            />
            <button type="submit" className="btn btn--icon" disabled={addMemberMutation.isPending}>
              <UserPlus size={16} />
            </button>
          </form>

          {members.length === 0 ? (
            <p className="empty-state">No members yet</p>
          ) : (
            <ul className="member-list">
              {members.map((m) => (
                <li key={m.id} className="member-list__item">
                  <span>{m.member_address}</span>
                  <button
                    className="btn btn--icon"
                    onClick={() => removeMemberMutation.mutate(m.member_address)}
                  >
                    <Trash2 size={14} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
