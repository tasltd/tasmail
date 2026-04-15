// Added: CalDAV/CardDAV configuration management UI for TMAIL-117
// PURPOSE: Allows users to configure CalDAV/CardDAV servers for calendar and contact sync
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Zap, RefreshCw } from 'lucide-react';
import {
  listDavConfigs,
  createDavConfig,
  updateDavConfig,
  deleteDavConfig,
  syncDavConfig,
  testDavConfig,
} from '../../api/dav-config';
import type { DavConfiguration, DavType } from '../../api/dav-config';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: DAV type options with display labels
const DAV_TYPE_OPTIONS: { value: DavType; label: string }[] = [
  { value: 'caldav', label: 'CalDAV (Calendars)' },
  { value: 'carddav', label: 'CardDAV (Contacts)' },
  { value: 'both', label: 'Both (Calendars + Contacts)' },
];

// Added: Quick presets for common CalDAV/CardDAV providers
const DAV_PRESETS: { label: string; url: string; type: DavType }[] = [
  { label: 'Radicale', url: 'https://radicale.example.com', type: 'both' },
  { label: 'Nextcloud', url: 'https://cloud.example.com/remote.php/dav', type: 'both' },
  { label: 'iCloud', url: 'https://caldav.icloud.com', type: 'caldav' },
  { label: 'Google', url: 'https://www.googleapis.com/.well-known/caldav', type: 'caldav' },
];

export function DavConfigManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Added: Form state for creating/editing DAV configurations
  const [formName, setFormName] = useState('');
  const [formServerUrl, setFormServerUrl] = useState('');
  const [formUsername, setFormUsername] = useState('');
  const [formPassword, setFormPassword] = useState('');
  const [formDavType, setFormDavType] = useState<DavType>('both');
  const [formSyncInterval, setFormSyncInterval] = useState(60);
  const [formEnabled, setFormEnabled] = useState(true);

  const { data: configs, isLoading } = useQuery({
    queryKey: ['dav-configs'],
    queryFn: listDavConfigs,
  });

  const createMut = useMutation({
    mutationFn: createDavConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dav-configs'] });
      setIsCreating(false);
      resetForm();
    },
  });

  const updateMut = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Parameters<typeof updateDavConfig>[1] }) =>
      updateDavConfig(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['dav-configs'] });
      setEditingId(null);
      resetForm();
    },
  });

  const deleteMut = useMutation({
    mutationFn: deleteDavConfig,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['dav-configs'] }),
  });

  const syncMut = useMutation({
    mutationFn: syncDavConfig,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['dav-configs'] }),
  });

  const testMut = useMutation({
    mutationFn: testDavConfig,
    onSuccess: (result) => {
      const msg = result.success
        ? `Connection successful (${result.latency_ms}ms)`
        : `Test failed: ${result.message}`;
      setTestResult(msg);
      setTestingId(null);
      queryClient.invalidateQueries({ queryKey: ['dav-configs'] });
    },
    onError: () => {
      setTestResult('Failed to test DAV configuration');
      setTestingId(null);
    },
  });

  // Added: Reset all form fields to defaults
  function resetForm() {
    setFormName('');
    setFormServerUrl('');
    setFormUsername('');
    setFormPassword('');
    setFormDavType('both');
    setFormSyncInterval(60);
    setFormEnabled(true);
  }

  // Added: Apply a preset configuration
  const handlePreset = (preset: (typeof DAV_PRESETS)[number]) => {
    setFormServerUrl(preset.url);
    setFormDavType(preset.type);
    if (!formName) setFormName(preset.label);
  };

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      name: formName,
      server_url: formServerUrl,
      username: formUsername,
      password: formPassword,
      dav_type: formDavType,
      sync_interval_minutes: formSyncInterval,
      enabled: formEnabled,
    });
  };

  const handleUpdate = (e: FormEvent) => {
    e.preventDefault();
    if (!editingId) return;
    updateMut.mutate({
      id: editingId,
      data: {
        name: formName || undefined,
        server_url: formServerUrl || undefined,
        username: formUsername || undefined,
        password: formPassword || undefined,
        dav_type: formDavType,
        sync_interval_minutes: formSyncInterval,
        enabled: formEnabled,
      },
    });
  };

  const handleEdit = (config: DavConfiguration) => {
    setEditingId(config.id);
    setIsCreating(false);
    setFormName(config.name);
    setFormServerUrl(config.server_url);
    setFormUsername(config.username);
    setFormPassword('');
    setFormDavType(config.dav_type as DavType);
    setFormSyncInterval(config.sync_interval_minutes);
    setFormEnabled(config.enabled);
  };

  const handleTest = (id: string) => {
    setTestingId(id);
    setTestResult(null);
    testMut.mutate(id);
  };

  // Added: Helper to get sync status badge color
  const getSyncStatusColor = (status: string | null) => {
    switch (status) {
      case 'syncing': return '#2196f3';
      case 'error': return '#f44336';
      default: return 'gray';
    }
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="dav-config-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>CalDAV / CardDAV</h2>
        <button
          className="btn btn--primary"
          onClick={() => { setIsCreating(true); setEditingId(null); resetForm(); }}
          data-testid="add-dav-btn"
        >
          <Plus size={16} /> Add DAV Server
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

      {/* Added: Create/edit DAV config form */}
      {(isCreating || editingId) && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
          data-testid="dav-form"
        >
          <h3 style={{ marginBottom: '12px' }}>
            {editingId ? 'Edit DAV Server' : 'New DAV Server'}
          </h3>
          {/* Added: Quick preset buttons for common providers */}
          {!editingId && (
            <div style={{ marginBottom: '12px', display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
              {DAV_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className="btn"
                  onClick={() => handlePreset(preset)}
                  style={{ fontSize: '12px', padding: '2px 8px' }}
                  data-testid={`preset-${preset.label.toLowerCase()}`}
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
                placeholder="e.g., Radicale, Nextcloud, iCloud"
                required={!editingId}
                data-testid="name-input"
              />
            </div>
            <div className="composer__field">
              <label>Server URL</label>
              <input
                value={formServerUrl}
                onChange={(e) => setFormServerUrl(e.target.value)}
                placeholder="https://radicale.example.com"
                required={!editingId}
                data-testid="server-url-input"
              />
            </div>
            <div style={{ display: 'flex', gap: '12px' }}>
              <div className="composer__field" style={{ flex: 1 }}>
                <label>Type</label>
                <select
                  value={formDavType}
                  onChange={(e) => setFormDavType(e.target.value as DavType)}
                  data-testid="dav-type-select"
                >
                  {DAV_TYPE_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
              <div className="composer__field" style={{ flex: 1 }}>
                <label>Sync Interval (minutes)</label>
                <input
                  type="number"
                  value={formSyncInterval}
                  onChange={(e) => setFormSyncInterval(parseInt(e.target.value, 10) || 60)}
                  min={5}
                  data-testid="sync-interval-input"
                />
              </div>
            </div>
            <div className="composer__field">
              <label>Username</label>
              <input
                value={formUsername}
                onChange={(e) => setFormUsername(e.target.value)}
                placeholder="DAV username or email"
                required={!editingId}
                data-testid="username-input"
              />
            </div>
            <div className="composer__field">
              <label>Password</label>
              <input
                value={formPassword}
                onChange={(e) => setFormPassword(e.target.value)}
                placeholder={editingId ? 'Leave blank to keep current' : 'DAV password or app password'}
                type="password"
                required={!editingId}
                data-testid="password-input"
              />
            </div>
            <div className="composer__field" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input
                type="checkbox"
                checked={formEnabled}
                onChange={(e) => setFormEnabled(e.target.checked)}
                id="dav-enabled"
                data-testid="enabled-checkbox"
              />
              <label htmlFor="dav-enabled">Enable automatic sync</label>
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" data-testid="dav-submit">
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
            No CalDAV/CardDAV servers configured. Add one to sync calendars and contacts.
          </p>
        )}
        {configs?.map((config: DavConfiguration) => (
          <div
            key={config.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
            data-testid={`config-${config.id}`}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <RefreshCw size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1, cursor: 'pointer' }} onClick={() => handleEdit(config)}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <span style={{ fontSize: '14px', fontWeight: 500 }}>{config.name}</span>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: config.dav_type === 'both' ? 'var(--color-primary)' : '#607d8b',
                      color: 'white',
                    }}
                    data-testid="type-badge"
                  >
                    {config.dav_type}
                  </span>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: config.enabled ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {config.enabled ? 'Enabled' : 'Disabled'}
                  </span>
                  {config.sync_status && config.sync_status !== 'idle' && (
                    <span
                      style={{
                        fontSize: '11px',
                        padding: '1px 6px',
                        borderRadius: '10px',
                        background: getSyncStatusColor(config.sync_status),
                        color: 'white',
                      }}
                      data-testid="sync-status-badge"
                    >
                      {config.sync_status}
                    </span>
                  )}
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {config.server_url} | {config.username} | Pass: {config.password_masked} | Every {config.sync_interval_minutes}min
                </div>
                {config.sync_error && (
                  <div style={{ fontSize: '11px', color: '#f44336', marginTop: '2px' }}>
                    Error: {config.sync_error}
                  </div>
                )}
              </div>
              {/* Added: Sync now button */}
              <button
                className="btn btn--icon"
                onClick={() => syncMut.mutate(config.id)}
                title="Sync now"
                data-testid={`sync-${config.id}`}
              >
                <RefreshCw size={16} />
              </button>
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
