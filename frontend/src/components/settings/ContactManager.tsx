import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, Edit2, ArrowLeft, Search } from 'lucide-react';
import {
  fetchContacts,
  createContact,
  updateContact,
  deleteContact,
} from '../../api/contacts';
import type { Contact, CreateContactRequest } from '../../api/contacts';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

function ContactEditor({
  contact,
  onSave,
  onCancel,
}: {
  contact?: Contact;
  onSave: (data: CreateContactRequest) => void;
  onCancel: () => void;
}) {
  const [email, setEmail] = useState(contact?.email || '');
  const [displayName, setDisplayName] = useState(contact?.display_name || '');
  const [company, setCompany] = useState(contact?.company || '');
  const [phone, setPhone] = useState(contact?.phone || '');
  const [notes, setNotes] = useState(contact?.notes || '');

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    onSave({
      email,
      display_name: displayName || undefined,
      company: company || undefined,
      phone: phone || undefined,
      notes: notes || undefined,
    });
  };

  return (
    <form className="composer__fields" onSubmit={handleSubmit} style={{ gap: '12px' }}>
      <div className="composer__field">
        <label>Email</label>
        <input value={email} onChange={(e) => setEmail(e.target.value)} placeholder="user@example.com" required type="email" />
      </div>
      <div className="composer__field">
        <label>Name</label>
        <input value={displayName} onChange={(e) => setDisplayName(e.target.value)} placeholder="Display name" />
      </div>
      <div className="composer__field">
        <label>Company</label>
        <input value={company} onChange={(e) => setCompany(e.target.value)} placeholder="Company" />
      </div>
      <div className="composer__field">
        <label>Phone</label>
        <input value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="Phone number" />
      </div>
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>Notes</label>
        <textarea
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          placeholder="Notes"
          rows={3}
          style={{ width: '100%', padding: '8px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontSize: '13px' }}
        />
      </div>
      <div className="composer__actions">
        <button type="submit" className="btn btn--primary">Save</button>
        <button type="button" className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </form>
  );
}

export function ContactManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [searchQuery, setSearchQuery] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const { data: contacts, isLoading } = useQuery({
    queryKey: ['contacts', searchQuery],
    queryFn: () => fetchContacts(searchQuery || undefined),
  });

  const createMut = useMutation({
    mutationFn: createContact,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contacts'] });
      setIsCreating(false);
    },
  });

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Parameters<typeof updateContact>[1] }) =>
      updateContact(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['contacts'] });
      setEditingId(null);
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteContact,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['contacts'] }),
  });

  if (isLoading) return <LoadingSkeleton rows={6} />;

  const editingContact = contacts?.find((c) => c.id === editingId);

  return (
    <div style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Contacts</h2>
        <button className="btn btn--primary" onClick={() => { setIsCreating(true); setEditingId(null); }}>
          <Plus size={16} /> Add Contact
        </button>
      </div>

      <div style={{ margin: '12px 0', display: 'flex', gap: '8px', alignItems: 'center', background: 'var(--color-bg)', padding: '6px 12px', borderRadius: '8px' }}>
        <Search size={16} />
        <input
          type="text"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search contacts..."
          style={{ flex: 1, border: 'none', background: 'none', outline: 'none', fontSize: '14px' }}
        />
      </div>

      {isCreating && (
        <div style={{ marginTop: '12px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>New Contact</h3>
          <ContactEditor onSave={(data) => createMut.mutate(data)} onCancel={() => setIsCreating(false)} />
        </div>
      )}

      {editingId && editingContact && (
        <div style={{ marginTop: '12px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>Edit Contact</h3>
          <ContactEditor
            contact={editingContact}
            onSave={(data) => updateMut.mutate({ id: editingId, data })}
            onCancel={() => setEditingId(null)}
          />
        </div>
      )}

      <div style={{ marginTop: '12px' }}>
        {(!contacts || contacts.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            {searchQuery ? 'No contacts match your search' : 'No contacts yet. Add one to get started.'}
          </p>
        )}
        {contacts?.map((contact) => (
          <div
            key={contact.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '10px 12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ width: '36px', height: '36px', borderRadius: '50%', background: 'var(--color-primary)', color: 'white', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '14px', fontWeight: 600, flexShrink: 0 }}>
              {(contact.display_name || contact.email)[0].toUpperCase()}
            </div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {contact.display_name || contact.email}
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
                {contact.email}{contact.company ? ` · ${contact.company}` : ''}
              </div>
            </div>
            <button className="btn btn--icon" onClick={() => { setEditingId(contact.id); setIsCreating(false); }} title="Edit">
              <Edit2 size={16} />
            </button>
            <button className="btn btn--icon btn--danger" onClick={() => deleteMut.mutate(contact.id)} title="Delete">
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
