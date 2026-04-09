import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, Check, Edit2, ArrowLeft } from 'lucide-react';
import {
  fetchSignatures,
  createSignature,
  updateSignature,
  deleteSignature,
} from '../../api/signatures';
import type { Signature } from '../../api/signatures';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

function SignatureEditor({
  signature,
  onSave,
  onCancel,
}: {
  signature?: Signature;
  onSave: (data: { name: string; html_body: string; text_body: string; is_default: boolean }) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(signature?.name || '');
  const [htmlBody, setHtmlBody] = useState(signature?.html_body || '');
  const [textBody, setTextBody] = useState(signature?.text_body || '');
  const [isDefault, setIsDefault] = useState(signature?.is_default || false);

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    onSave({ name, html_body: htmlBody, text_body: textBody, is_default: isDefault });
  };

  return (
    <form className="signature-editor" onSubmit={handleSubmit}>
      <div className="composer__field">
        <label>Name</label>
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Signature name" required />
      </div>
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>HTML Body</label>
        <textarea
          value={htmlBody}
          onChange={(e) => setHtmlBody(e.target.value)}
          placeholder="<p>Best regards,<br/>Your Name</p>"
          rows={8}
          style={{ width: '100%', padding: '8px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontFamily: 'monospace', fontSize: '13px' }}
        />
      </div>
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>Plain Text</label>
        <textarea
          value={textBody}
          onChange={(e) => setTextBody(e.target.value)}
          placeholder="Best regards,&#10;Your Name"
          rows={4}
          style={{ width: '100%', padding: '8px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontSize: '13px' }}
        />
      </div>
      <label style={{ display: 'flex', gap: '8px', alignItems: 'center', fontSize: '13px', margin: '8px 0' }}>
        <input type="checkbox" checked={isDefault} onChange={(e) => setIsDefault(e.target.checked)} />
        Set as default signature
      </label>
      <div className="composer__actions">
        <button type="submit" className="btn btn--primary">
          <Check size={16} /> Save
        </button>
        <button type="button" className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </form>
  );
}

export function SignatureManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);

  const { data: signatures, isLoading } = useQuery({
    queryKey: ['signatures'],
    queryFn: fetchSignatures,
  });

  const createMut = useMutation({
    mutationFn: createSignature,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['signatures'] });
      setIsCreating(false);
    },
  });

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Parameters<typeof updateSignature>[1] }) =>
      updateSignature(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['signatures'] });
      setEditingId(null);
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteSignature,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['signatures'] }),
  });

  if (isLoading) return <LoadingSkeleton rows={4} />;

  const editingSignature = signatures?.find((s) => s.id === editingId);

  return (
    <div className="signature-manager" style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Email Signatures</h2>
        <button className="btn btn--primary" onClick={() => { setIsCreating(true); setEditingId(null); }}>
          <Plus size={16} /> New Signature
        </button>
      </div>

      {isCreating && (
        <div style={{ marginTop: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>New Signature</h3>
          <SignatureEditor
            onSave={(data) => createMut.mutate(data)}
            onCancel={() => setIsCreating(false)}
          />
        </div>
      )}

      {editingId && editingSignature && (
        <div style={{ marginTop: '16px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>Edit Signature</h3>
          <SignatureEditor
            signature={editingSignature}
            onSave={(data) => updateMut.mutate({ id: editingId, data })}
            onCancel={() => setEditingId(null)}
          />
        </div>
      )}

      <div style={{ marginTop: '16px' }}>
        {(!signatures || signatures.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No signatures yet. Create one to get started.
          </p>
        )}
        {signatures?.map((sig) => (
          <div
            key={sig.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ flex: 1 }}>
              <strong>{sig.name}</strong>
              {sig.is_default && (
                <span style={{ marginLeft: '8px', fontSize: '11px', background: 'var(--color-primary)', color: 'white', padding: '1px 6px', borderRadius: '10px' }}>
                  Default
                </span>
              )}
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '4px' }}>
                {sig.text_body.slice(0, 80)}{sig.text_body.length > 80 ? '...' : ''}
              </div>
            </div>
            <button className="btn btn--icon" onClick={() => { setEditingId(sig.id); setIsCreating(false); }} title="Edit">
              <Edit2 size={16} />
            </button>
            <button className="btn btn--icon btn--danger" onClick={() => deleteMut.mutate(sig.id)} title="Delete">
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
