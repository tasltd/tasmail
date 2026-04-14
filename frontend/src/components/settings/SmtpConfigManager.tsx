// Added: SMTP configuration management UI for BYO-SMTP (TMAIL-48)
// PURPOSE: Allows users to configure their own external SMTP servers for sending emails
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Star, Zap, Send } from 'lucide-react';
import {
  listSmtpConfigs,
  createSmtpConfig,
  updateSmtpConfig,
  deleteSmtpConfig,
  testSmtpConfig,
  setDefaultSmtp,
} from '../../api/smtp-config';
import type { SmtpConfiguration, SmtpEncryption } from '../../api/smtp-config';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: Encryption options with display labels
const ENCRYPTION_OPTIONS: { value: SmtpEncryption; label: string }[] = [
  { value: 'starttls', label: 'STARTTLS (port 587)' },
  { value: 'ssl', label: 'SSL/TLS (port 465)' },
  { value: 'none', label: 'None (port 25)' },
];

// Added: Common SMTP presets for quick configuration
const SMTP_PRESETS: { label: string; host: string; port: number; encryption: SmtpEncryption }[] = [
  { label: 'Gmail', host: 'smtp.gmail.com', port: 587, encryption: 'starttls' },
  { label: 'Outlook/Hotmail', host: 'smtp.office365.com', port: 587, encryption: 'starttls' },
  { label: 'Yahoo', host: 'smtp.mail.yahoo.com', port: 465, encryption: 'ssl' },
  { label: 'SendGrid', host: 'smtp.sendgrid.net', port: 587, encryption: 'starttls' },
  { label: 'Amazon SES', host: 'email-smtp.us-east-1.amazonaws.com', port: 587, encryption: 'starttls' },
];

export function SmtpConfigManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Added: Form state for creating/editing SMTP configurations
  const [formName, setFormName] = useState('');
  const [formHost, setFormHost] = useState('');
  const [formPort, setFormPort] = useState(587);
  const [formUsername, setFormUsername] = useState('');
  const [formPassword, setFormPassword] = useState('');
  const [formEncryption, setFormEncryption] = useState<SmtpEncryption>('starttls');
  const [formFromAddress, setFormFromAddress] = useState('');

  const { data: configs, isLoading } = useQuery({
    queryKey: ['smtp-configs'],
    queryFn: listSmtpConfigs,
  });

  const createMut = useMutation({
    mutationFn: createSmtpConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['smtp-configs'] });
      setIsCreating(false);
      resetForm();
    },
  });

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Parameters<typeof updateSmtpConfig>[1] }) =>
      updateSmtpConfig(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['smtp-configs'] });
      setEditingId(null);
      resetForm();
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteSmtpConfig,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['smtp-configs'] }),
  });

  const testMut = useMutation({
    mutationFn: testSmtpConfig,
    onSuccess: (result) => {
      const msg = result.success
        ? `Connection successful (${result.latency_ms}ms)`
        : `Test failed: ${result.message}`;
      setTestResult(msg);
      setTestingId(null);
      queryClient.invalidateQueries({ queryKey: ['smtp-configs'] });
    },
    onError: () => {
      setTestResult('Failed to test SMTP configuration');
      setTestingId(null);
    },
  });

  const defaultMut = useMutation({
    mutationFn: setDefaultSmtp,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['smtp-configs'] }),
  });

  // Added: Reset all form fields to defaults
  function resetForm() {
    setFormName('');
    setFormHost('');
    setFormPort(587);
    setFormUsername('');
    setFormPassword('');
    setFormEncryption('starttls');
    setFormFromAddress('');
  }

  // Added: Apply a preset configuration
  const handlePreset = (preset: (typeof SMTP_PRESETS)[number]) => {
    setFormHost(preset.host);
    setFormPort(preset.port);
    setFormEncryption(preset.encryption);
    if (!formName) setFormName(preset.label);
  };

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      name: formName,
      host: formHost,
      port: formPort,
      username: formUsername,
      password: formPassword,
      encryption: formEncryption,
      from_address: formFromAddress || undefined,
    });
  };

  const handleUpdate = (e: FormEvent) => {
    e.preventDefault();
    if (!editingId) return;
    updateMut.mutate({
      id: editingId,
      data: {
        name: formName || undefined,
        host: formHost || undefined,
        port: formPort,
        username: formUsername || undefined,
        password: formPassword || undefined,
        encryption: formEncryption,
        from_address: formFromAddress || undefined,
      },
    });
  };

  const handleEdit = (config: SmtpConfiguration) => {
    setEditingId(config.id);
    setIsCreating(false);
    setFormName(config.name);
    setFormHost(config.host);
    setFormPort(config.port);
    setFormUsername(config.username);
    setFormPassword('');
    setFormEncryption(config.encryption as SmtpEncryption);
    setFormFromAddress(config.from_address || '');
  };

  const handleTest = (id: string) => {
    setTestingId(id);
    setTestResult(null);
    testMut.mutate(id);
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="smtp-config-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>SMTP Configuration</h2>
        <button
          className="btn btn--primary"
          onClick={() => { setIsCreating(true); setEditingId(null); resetForm(); }}
          data-testid="add-smtp-btn"
        >
          <Plus size={16} /> Add SMTP Server
        </button>
      </div>

      {/* Added: Test result banner */}
      {testResult && (
        <div
          style={{
            marginTop: '12px',
            padding: '8px 12px',
            borderRadius: '6px',
            background: 'var(--color-surface)',
            border: '1px solid var(--color-border)',
            fontSize: '13px',
          }}
          data-testid="test-result"
        >
          {testResult}
          <button
            className="btn btn--icon"
            onClick={() => setTestResult(null)}
            style={{ marginLeft: '8px', fontSize: '12px' }}
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Added: Create/edit SMTP config form */}
      {(isCreating || editingId) && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
          data-testid="smtp-form"
        >
          <h3 style={{ marginBottom: '12px' }}>
            {editingId ? 'Edit SMTP Server' : 'New SMTP Server'}
          </h3>
          {/* Added: Quick preset buttons for common providers */}
          {!editingId && (
            <div style={{ marginBottom: '12px', display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
              {SMTP_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className="btn"
                  onClick={() => handlePreset(preset)}
                  style={{ fontSize: '12px', padding: '2px 8px' }}
                  data-testid={`preset-${preset.label.toLowerCase().replace(/[/ ]/g, '-')}`}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          )}
          <form onSubmit={editingId ? handleUpdate : handleCreate}>
            <div className="composer__field">
              <label>Name</label>
              <input
                value={formName}
                onChange={(e) => setFormName(e.target.value)}
                placeholder="e.g., Gmail, SendGrid, Work SMTP"
                required={!editingId}
                data-testid="name-input"
              />
            </div>
            <div className="composer__field">
              <label>Host</label>
              <input
                value={formHost}
                onChange={(e) => setFormHost(e.target.value)}
                placeholder="smtp.example.com"
                required={!editingId}
                data-testid="host-input"
              />
            </div>
            <div style={{ display: 'flex', gap: '12px' }}>
              <div className="composer__field" style={{ flex: 1 }}>
                <label>Port</label>
                <input
                  type="number"
                  value={formPort}
                  onChange={(e) => setFormPort(parseInt(e.target.value, 10) || 587)}
                  data-testid="port-input"
                />
              </div>
              <div className="composer__field" style={{ flex: 1 }}>
                <label>Encryption</label>
                <select
                  value={formEncryption}
                  onChange={(e) => setFormEncryption(e.target.value as SmtpEncryption)}
                  data-testid="encryption-select"
                >
                  {ENCRYPTION_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <div className="composer__field">
              <label>Username</label>
              <input
                value={formUsername}
                onChange={(e) => setFormUsername(e.target.value)}
                placeholder="SMTP username or email"
                required={!editingId}
                data-testid="username-input"
              />
            </div>
            <div className="composer__field">
              <label>Password</label>
              <input
                value={formPassword}
                onChange={(e) => setFormPassword(e.target.value)}
                placeholder={editingId ? 'Leave blank to keep current' : 'SMTP password or app password'}
                type="password"
                required={!editingId}
                data-testid="password-input"
              />
            </div>
            <div className="composer__field">
              <label>From Address (optional)</label>
              <input
                value={formFromAddress}
                onChange={(e) => setFormFromAddress(e.target.value)}
                placeholder="sender@example.com"
                type="email"
                data-testid="from-address-input"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" data-testid="smtp-submit">
                {editingId ? 'Update' : 'Save Configuration'}
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => { setIsCreating(false); setEditingId(null); resetForm(); }}
              >
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Config list */}
      <div style={{ marginTop: '16px' }}>
        {(!configs || configs.length === 0) && !isCreating && !editingId && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No SMTP servers configured. Add one to send emails through your own provider.
          </p>
        )}
        {configs?.map((config: SmtpConfiguration) => (
          <div
            key={config.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
            data-testid={`config-${config.id}`}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <Send size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1, cursor: 'pointer' }} onClick={() => handleEdit(config)}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span style={{ fontSize: '14px', fontWeight: 500 }}>{config.name}</span>
                  {config.is_default && (
                    <span
                      style={{
                        fontSize: '11px',
                        padding: '1px 6px',
                        borderRadius: '10px',
                        background: 'var(--color-primary)',
                        color: 'white',
                        fontWeight: 'bold',
                      }}
                      data-testid="default-badge"
                    >
                      Default
                    </span>
                  )}
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: config.verified ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {config.verified ? 'Verified' : 'Unverified'}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {config.host}:{config.port} ({config.encryption}) | {config.username} | Pass: {config.password_masked}
                </div>
              </div>
              {/* Added: Test connection button */}
              <button
                className="btn btn--icon"
                onClick={() => handleTest(config.id)}
                title="Test connection"
                disabled={testingId === config.id}
                data-testid={`test-${config.id}`}
              >
                <Zap size={16} />
              </button>
              {/* Added: Set as default button */}
              {!config.is_default && (
                <button
                  className="btn btn--icon"
                  onClick={() => defaultMut.mutate(config.id)}
                  title="Set as default"
                  data-testid={`default-${config.id}`}
                >
                  <Star size={16} />
                </button>
              )}
              {/* Added: Delete button */}
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(config.id)}
                title="Delete"
                data-testid={`delete-${config.id}`}
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
