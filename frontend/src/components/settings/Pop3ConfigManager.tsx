// Added: POP3 configuration management UI for Dovecot POP3 access (TMAIL-133)
// PURPOSE: Allows users to enable/configure POP3 access and view connection info for mail clients
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Download, Trash2, Info } from 'lucide-react';
import {
  getPop3Config,
  updatePop3Config,
  deletePop3Config,
  getPop3Status,
} from '../../api/pop3-config';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

export function Pop3ConfigManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [error, setError] = useState<string | null>(null);

  // Added: Form state for POP3 settings
  const [formEnabled, setFormEnabled] = useState(false);
  const [formDeleteAfterDownload, setFormDeleteAfterDownload] = useState(false);
  const [formRetentionDays, setFormRetentionDays] = useState<string>('');

  const { data: config, isLoading } = useQuery({
    queryKey: ['pop3-config'],
    queryFn: getPop3Config,
    // Added: Populate form when config loads
    select: (data) => {
      if (data && !formInitialized) {
        setFormEnabled(data.enabled);
        setFormDeleteAfterDownload(data.delete_after_download);
        setFormRetentionDays(data.retention_days != null ? String(data.retention_days) : '');
        setFormInitialized(true);
      }
      return data;
    },
  });

  const { data: status } = useQuery({
    queryKey: ['pop3-status'],
    queryFn: getPop3Status,
  });

  const [formInitialized, setFormInitialized] = useState(false);

  const updateMut = useMutation({
    mutationFn: updatePop3Config,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['pop3-config'] });
      setError(null);
    },
    onError: () => setError('Failed to update POP3 configuration'),
  });

  const deleteMut = useMutation({
    mutationFn: deletePop3Config,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['pop3-config'] });
      setFormEnabled(false);
      setFormDeleteAfterDownload(false);
      setFormRetentionDays('');
      setFormInitialized(false);
      setError(null);
    },
    onError: () => setError('Failed to delete POP3 configuration'),
  });

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const retentionDays = formRetentionDays.trim()
      ? parseInt(formRetentionDays, 10)
      : null;

    // Added: Client-side validation for retention days
    if (retentionDays !== null && (isNaN(retentionDays) || retentionDays <= 0)) {
      setError('Retention days must be a positive number');
      return;
    }

    updateMut.mutate({
      enabled: formEnabled,
      delete_after_download: formDeleteAfterDownload,
      retention_days: retentionDays,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="pop3-config-manager" style={{ padding: '16px', maxWidth: '700px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>
          <Download size={20} style={{ verticalAlign: 'middle', marginRight: '8px' }} />
          POP3 Configuration
        </h2>
      </div>

      {/* Added: Error banner */}
      {error && (
        <div
          style={{
            marginTop: '12px',
            padding: '8px 12px',
            borderRadius: '6px',
            background: 'var(--color-surface)',
            border: '1px solid var(--color-border)',
            fontSize: '13px',
            color: 'var(--color-danger, red)',
          }}
          data-testid="error-banner"
        >
          {error}
          <button
            className="btn btn--icon"
            onClick={() => setError(null)}
            style={{ marginLeft: '8px', fontSize: '12px' }}
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Added: POP3 connection info display */}
      {status && (
        <div
          style={{
            marginTop: '16px',
            padding: '12px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            background: 'var(--color-surface)',
          }}
          data-testid="connection-info"
        >
          <h3 style={{ fontSize: '14px', marginBottom: '8px', display: 'flex', alignItems: 'center', gap: '6px' }}>
            <Info size={16} />
            Mail Client Connection Info
          </h3>
          <div style={{ fontSize: '13px', display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px' }}>
            <span style={{ fontWeight: 500 }}>Server:</span>
            <span data-testid="pop3-server">{status.server}</span>
            <span style={{ fontWeight: 500 }}>Port:</span>
            <span data-testid="pop3-port">{status.port}</span>
            <span style={{ fontWeight: 500 }}>Encryption:</span>
            <span data-testid="pop3-encryption">{status.encryption}</span>
            <span style={{ fontWeight: 500 }}>Username:</span>
            <span data-testid="pop3-username">{status.username_format}</span>
          </div>
        </div>
      )}

      {/* Added: POP3 settings form */}
      <form
        onSubmit={handleSubmit}
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
        }}
        data-testid="pop3-form"
      >
        <h3 style={{ marginBottom: '12px' }}>POP3 Settings</h3>

        <div className="composer__field" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <input
            type="checkbox"
            id="pop3-enabled"
            checked={formEnabled}
            onChange={(e) => setFormEnabled(e.target.checked)}
            data-testid="enabled-toggle"
          />
          <label htmlFor="pop3-enabled" style={{ fontWeight: 500 }}>
            Enable POP3 access
          </label>
        </div>

        <div className="composer__field" style={{ display: 'flex', alignItems: 'center', gap: '8px', marginTop: '8px' }}>
          <input
            type="checkbox"
            id="pop3-delete-after-download"
            checked={formDeleteAfterDownload}
            onChange={(e) => setFormDeleteAfterDownload(e.target.checked)}
            data-testid="delete-after-download-toggle"
          />
          <label htmlFor="pop3-delete-after-download">
            Delete messages after download
          </label>
        </div>

        <div className="composer__field" style={{ marginTop: '12px' }}>
          <label>Retention days (leave empty for unlimited)</label>
          <input
            type="number"
            value={formRetentionDays}
            onChange={(e) => setFormRetentionDays(e.target.value)}
            placeholder="e.g., 30"
            min="1"
            style={{ maxWidth: '200px' }}
            data-testid="retention-days-input"
          />
        </div>

        <div className="composer__actions" style={{ marginTop: '16px', display: 'flex', gap: '8px' }}>
          <button type="submit" className="btn btn--primary" data-testid="save-btn">
            Save Configuration
          </button>
          {config && (
            <button
              type="button"
              className="btn btn--danger"
              onClick={() => deleteMut.mutate()}
              data-testid="delete-btn"
            >
              <Trash2 size={16} /> Delete POP3 Config
            </button>
          )}
        </div>
      </form>

      {/* Added: Last POP3 login info */}
      {config?.last_pop3_login && (
        <div style={{ marginTop: '12px', fontSize: '12px', color: 'var(--color-text-secondary)' }}>
          Last POP3 login: {new Date(config.last_pop3_login).toLocaleString()}
        </div>
      )}
    </div>
  );
}
