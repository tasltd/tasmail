// Added: Full contacts management app with groups, import/export, merge (TMAIL-119)
import { useState } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  BookUser, Plus, Trash2, Upload, Download, Merge, Tag, X, Search,
} from 'lucide-react';
import { fetchContacts } from '../../api/contacts';
import type { Contact } from '../../api/contacts';
import {
  listContactGroups,
  createContactGroup,
  deleteContactGroup,
  addContactToGroup,
  removeContactFromGroup,
  listContactsInGroup,
  importVcard,
  exportVcard,
  mergeContacts,
} from '../../api/contact-groups';
import type { ContactGroup } from '../../api/contact-groups';

export function ContactsApp() {
  const queryClient = useQueryClient();
  const [selectedGroupId, setSelectedGroupId] = useState<string | null>(null);
  const [selectedContactId, setSelectedContactId] = useState<string | null>(null);
  const [searchFilter, setSearchFilter] = useState('');
  const [showCreateGroup, setShowCreateGroup] = useState(false);
  const [newGroupName, setNewGroupName] = useState('');
  const [newGroupColor, setNewGroupColor] = useState('#3b82f6');
  const [showImport, setShowImport] = useState(false);
  const [importText, setImportText] = useState('');

  // PURPOSE: Fetch all contacts
  const { data: allContacts = [], isLoading: loadingContacts } = useQuery({
    queryKey: ['contacts-app-contacts'],
    queryFn: () => fetchContacts(),
  });

  // PURPOSE: Fetch contact groups
  const { data: groups = [], isLoading: loadingGroups } = useQuery({
    queryKey: ['contact-groups'],
    queryFn: listContactGroups,
  });

  // PURPOSE: Fetch contacts in selected group
  const { data: groupContacts = [] } = useQuery({
    queryKey: ['contact-group-contacts', selectedGroupId],
    queryFn: () => listContactsInGroup(selectedGroupId!),
    enabled: !!selectedGroupId,
  });

  // NOTE: Filter contacts based on search and selected group
  const displayContacts = selectedGroupId ? groupContacts : allContacts;
  const filteredContacts = searchFilter
    ? displayContacts.filter(
        (c) =>
          c.email.toLowerCase().includes(searchFilter.toLowerCase()) ||
          (c.display_name && c.display_name.toLowerCase().includes(searchFilter.toLowerCase())) ||
          (c.company && c.company.toLowerCase().includes(searchFilter.toLowerCase()))
      )
    : displayContacts;

  const selectedContact = allContacts.find((c) => c.id === selectedContactId) || null;

  // PURPOSE: Group CRUD mutations
  const createGroupMutation = useMutation({
    mutationFn: () => createContactGroup({ name: newGroupName, color: newGroupColor }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contact-groups'] });
      setShowCreateGroup(false);
      setNewGroupName('');
      setNewGroupColor('#3b82f6');
    },
  });

  const deleteGroupMutation = useMutation({
    mutationFn: deleteContactGroup,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contact-groups'] });
      setSelectedGroupId(null);
    },
  });

  // PURPOSE: Import vCard mutation
  const importMutation = useMutation({
    mutationFn: () => importVcard(importText),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contacts-app-contacts'] });
      setShowImport(false);
      setImportText('');
    },
  });

  // PURPOSE: Merge duplicates — find contacts sharing the same email
  const findDuplicates = (): Map<string, Contact[]> => {
    const emailMap = new Map<string, Contact[]>();
    for (const c of allContacts) {
      const key = c.email.toLowerCase();
      const existing = emailMap.get(key) || [];
      existing.push(c);
      emailMap.set(key, existing);
    }
    // NOTE: Only return entries with 2+ contacts
    const dupes = new Map<string, Contact[]>();
    for (const [email, contacts] of emailMap) {
      if (contacts.length > 1) {
        dupes.set(email, contacts);
      }
    }
    return dupes;
  };

  const mergeMutation = useMutation({
    mutationFn: (ids: string[]) => mergeContacts(ids),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contacts-app-contacts'] });
    },
  });

  // PURPOSE: Export all contacts as vCard file download
  const handleExport = async () => {
    try {
      const vcardText = await exportVcard();
      const blob = new Blob([vcardText], { type: 'text/vcard' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = 'contacts.vcf';
      a.click();
      URL.revokeObjectURL(url);
    } catch {
      // NOTE: Error handled silently — could add toast notification
    }
  };

  const handleCreateGroup = (e: FormEvent) => {
    e.preventDefault();
    if (!newGroupName.trim()) return;
    createGroupMutation.mutate();
  };

  const handleImport = (e: FormEvent) => {
    e.preventDefault();
    if (!importText.trim()) return;
    importMutation.mutate();
  };

  const handleMergeAll = () => {
    const dupes = findDuplicates();
    for (const [, contacts] of dupes) {
      const ids = contacts.map((c) => c.id);
      mergeMutation.mutate(ids);
    }
  };

  const duplicateCount = findDuplicates().size;

  if (loadingContacts || loadingGroups) {
    return <div className="settings-panel"><p>Loading contacts...</p></div>;
  }

  return (
    <div className="settings-panel" style={{ display: 'flex', gap: '16px', height: '100%' }}>
      {/* Left panel: Groups sidebar */}
      <div style={{ width: '240px', flexShrink: 0, borderRight: '1px solid var(--color-border)', paddingRight: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
          <h3 style={{ margin: 0, display: 'flex', alignItems: 'center', gap: '6px' }}>
            <BookUser size={18} /> Contacts
          </h3>
        </div>

        {/* Action buttons */}
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginBottom: '12px' }}>
          <button className="btn btn--sm" onClick={() => setShowImport(true)} title="Import vCard">
            <Upload size={14} /> Import
          </button>
          <button className="btn btn--sm" onClick={handleExport} title="Export vCard">
            <Download size={14} /> Export
          </button>
          {duplicateCount > 0 && (
            <button className="btn btn--sm" onClick={handleMergeAll} title="Merge duplicates">
              <Merge size={14} /> Merge ({duplicateCount})
            </button>
          )}
        </div>

        {/* All Contacts button */}
        <button
          className={`folder-item ${selectedGroupId === null ? 'folder-item--active' : ''}`}
          onClick={() => { setSelectedGroupId(null); setSelectedContactId(null); }}
          style={{ width: '100%', textAlign: 'left' }}
        >
          <BookUser size={16} />
          <span className="folder-item__name">All Contacts ({allContacts.length})</span>
        </button>

        {/* Group list */}
        {groups.map((g: ContactGroup) => (
          <div key={g.id} style={{ display: 'flex', alignItems: 'center' }}>
            <button
              className={`folder-item ${selectedGroupId === g.id ? 'folder-item--active' : ''}`}
              onClick={() => { setSelectedGroupId(g.id); setSelectedContactId(null); }}
              style={{ flex: 1, textAlign: 'left' }}
            >
              <Tag size={16} style={{ color: g.color || undefined }} />
              <span className="folder-item__name">{g.name}</span>
            </button>
            <button
              className="btn btn--icon btn--sm"
              onClick={() => deleteGroupMutation.mutate(g.id)}
              title="Delete group"
              style={{ flexShrink: 0 }}
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}

        {/* Create group form */}
        {showCreateGroup ? (
          <form onSubmit={handleCreateGroup} style={{ marginTop: '8px' }}>
            <input
              type="text"
              className="input"
              placeholder="Group name"
              value={newGroupName}
              onChange={(e) => setNewGroupName(e.target.value)}
              style={{ marginBottom: '4px', width: '100%' }}
            />
            <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
              <input
                type="color"
                value={newGroupColor}
                onChange={(e) => setNewGroupColor(e.target.value)}
                style={{ width: '32px', height: '28px', padding: 0, border: 'none' }}
              />
              <button type="submit" className="btn btn--primary btn--sm" disabled={createGroupMutation.isPending}>
                Create
              </button>
              <button type="button" className="btn btn--sm" onClick={() => setShowCreateGroup(false)}>
                <X size={14} />
              </button>
            </div>
          </form>
        ) : (
          <button
            className="btn btn--sm"
            onClick={() => setShowCreateGroup(true)}
            style={{ marginTop: '8px', width: '100%' }}
          >
            <Plus size={14} /> New Group
          </button>
        )}
      </div>

      {/* Right panel: Contact list and detail */}
      <div style={{ flex: 1, minWidth: 0 }}>
        {/* Search bar */}
        <div style={{ marginBottom: '12px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <Search size={16} />
          <input
            type="text"
            className="input"
            placeholder="Search contacts..."
            value={searchFilter}
            onChange={(e) => setSearchFilter(e.target.value)}
            style={{ flex: 1 }}
          />
        </div>

        {/* Import vCard dialog */}
        {showImport && (
          <div style={{ marginBottom: '12px', padding: '12px', border: '1px solid var(--color-border)', borderRadius: '6px' }}>
            <h4 style={{ margin: '0 0 8px' }}>Import vCard</h4>
            <form onSubmit={handleImport}>
              <textarea
                className="input"
                placeholder="Paste vCard text here (BEGIN:VCARD ... END:VCARD)"
                value={importText}
                onChange={(e) => setImportText(e.target.value)}
                rows={6}
                style={{ width: '100%', marginBottom: '8px' }}
              />
              <div style={{ display: 'flex', gap: '8px' }}>
                <button type="submit" className="btn btn--primary btn--sm" disabled={importMutation.isPending}>
                  {importMutation.isPending ? 'Importing...' : 'Import'}
                </button>
                <button type="button" className="btn btn--sm" onClick={() => setShowImport(false)}>
                  Cancel
                </button>
              </div>
            </form>
          </div>
        )}

        {/* Contact list or detail */}
        {selectedContact ? (
          <ContactDetail
            contact={selectedContact}
            groups={groups}
            onBack={() => setSelectedContactId(null)}
            onAddToGroup={(groupId) => {
              addContactToGroup(groupId, selectedContact.id).then(() => {
                queryClient.invalidateQueries({ queryKey: ['contact-group-contacts', groupId] });
              });
            }}
            onRemoveFromGroup={(groupId) => {
              removeContactFromGroup(groupId, selectedContact.id).then(() => {
                queryClient.invalidateQueries({ queryKey: ['contact-group-contacts', groupId] });
              });
            }}
          />
        ) : (
          <ContactList contacts={filteredContacts} onSelect={(id) => setSelectedContactId(id)} />
        )}
      </div>
    </div>
  );
}

// PURPOSE: Renders a list of contacts with click-to-select
function ContactList({ contacts, onSelect }: { contacts: Contact[]; onSelect: (id: string) => void }) {
  if (contacts.length === 0) {
    return <p style={{ color: 'var(--color-text-secondary)' }}>No contacts found.</p>;
  }

  return (
    <div>
      {contacts.map((c) => (
        <button
          key={c.id}
          className="folder-item"
          onClick={() => onSelect(c.id)}
          style={{ width: '100%', textAlign: 'left', display: 'flex', justifyContent: 'space-between' }}
        >
          <div>
            <strong>{c.display_name || c.email}</strong>
            {c.display_name && <span style={{ marginLeft: '8px', color: 'var(--color-text-secondary)' }}>{c.email}</span>}
          </div>
          {c.company && <span style={{ color: 'var(--color-text-secondary)', fontSize: '0.85em' }}>{c.company}</span>}
        </button>
      ))}
    </div>
  );
}

// PURPOSE: Detail view for a single contact
function ContactDetail({
  contact,
  groups,
  onBack,
  onAddToGroup,
  onRemoveFromGroup: _onRemoveFromGroup,
}: {
  contact: Contact;
  groups: ContactGroup[];
  onBack: () => void;
  onAddToGroup: (groupId: string) => void;
  onRemoveFromGroup: (groupId: string) => void;
}) {
  return (
    <div>
      <button className="btn btn--sm" onClick={onBack} style={{ marginBottom: '12px' }}>
        &larr; Back to list
      </button>
      <div style={{ padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
        <h3 style={{ marginTop: 0 }}>{contact.display_name || contact.email}</h3>
        <div style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: '8px', lineHeight: 1.8 }}>
          <span style={{ fontWeight: 600 }}>Email:</span>
          <span>{contact.email}</span>
          {contact.phone && (
            <>
              <span style={{ fontWeight: 600 }}>Phone:</span>
              <span>{contact.phone}</span>
            </>
          )}
          {contact.company && (
            <>
              <span style={{ fontWeight: 600 }}>Company:</span>
              <span>{contact.company}</span>
            </>
          )}
          {contact.notes && (
            <>
              <span style={{ fontWeight: 600 }}>Notes:</span>
              <span>{contact.notes}</span>
            </>
          )}
        </div>

        {/* Group membership management */}
        {groups.length > 0 && (
          <div style={{ marginTop: '16px' }}>
            <h4>Groups</h4>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
              {groups.map((g) => (
                <button
                  key={g.id}
                  className="btn btn--sm"
                  onClick={() => onAddToGroup(g.id)}
                  title={`Add to ${g.name}`}
                  style={{ borderLeft: `3px solid ${g.color || '#888'}` }}
                >
                  <Tag size={12} /> {g.name}
                </button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
