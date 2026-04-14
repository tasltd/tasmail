// Added: OIDC provider management component for TMAIL-99
import { useState } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { LogIn, Plus, Save, Trash2, ArrowLeft } from 'lucide-react';
import {
  listOidcProviders,
  createOidcProvider,
  updateOidcProvider,
  deleteOidcProvider,
} from '../../api/oidc';
import type {
  OidcProvider,
  CreateOidcProviderRequest,
} from '../../api/oidc';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Admin UI for managing OIDC identity provider configurations (Google, Microsoft, etc.)
 * CONSTRAINTS: Only admins should access — route protection handled by backend
 * EXTERNAL: Uses /api/admin/oidc endpoints for CRUD operations
 */
export function OidcManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [error, setError] = useState('');

  // Added: Form state for OIDC provider fields
  const [name, setName] = useState('');
  const [issuerUrl, setIssuerUrl] = useState('');
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [scopes, setScopes] = useState('openid email profile');
  const [redirectUri, setRedirectUri] = useState('');
  const [autoCreateUsers, setAutoCreateUsers] = useState(false);
  const [defaultRole, setDefaultRole] = useState('user');
  const [iconUrl, setIconUrl] = useState('');
  const [buttonLabel, setButtonLabel] = useState('');

  // Added: Fetch all OIDC providers
  const { data: providers, isLoading } = useQuery<OidcProvider[]>({
    queryKey: ['oidc-providers'],
    queryFn: listOidcProviders,
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateOidcProviderRequest) => createOidcProvider(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['oidc-providers'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Record<string, unknown> }) =>
      updateOidcProvider(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['oidc-providers'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteOidcProvider(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['oidc-providers'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  // Added: Toggle active status via update mutation
  const toggleActiveMutation = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      updateOidcProvider(id, { active }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['oidc-providers'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  // Added: Reset form to initial state
  const resetForm = () => {
    setShowForm(false);
    setEditingId(null);
    setName('');
    setIssuerUrl('');
    setClientId('');
    setClientSecret('');
    setScopes('openid email profile');
    setRedirectUri('');
    setAutoCreateUsers(false);
    setDefaultRole('user');
    setIconUrl('');
    setButtonLabel('');
  };

  // Added: Populate form for editing an existing provider
  const startEditing = (provider: OidcProvider) => {
    setEditingId(provider.id);
    setShowForm(true);
    setName(provider.name);
    setIssuerUrl(provider.issuer_url);
    setClientId(provider.client_id);
    setClientSecret('');
    setScopes(provider.scopes);
    setRedirectUri(provider.redirect_uri);
    setAutoCreateUsers(provider.auto_create_users);
    setDefaultRole(provider.default_role);
    setIconUrl(provider.icon_url ?? '');
    setButtonLabel(provider.button_label ?? '');
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    const formData = {
      name,
      issuer_url: issuerUrl,
      client_id: clientId,
      scopes,
      redirect_uri: redirectUri,
      auto_create_users: autoCreateUsers,
      default_role: defaultRole,
      icon_url: iconUrl || undefined,
      button_label: buttonLabel || undefined,
    };

    if (editingId) {
      // Added: Only include client_secret if user entered a new one
      updateMutation.mutate({
        id: editingId,
        data: {
          ...formData,
          ...(clientSecret ? { client_secret: clientSecret } : {}),
        },
      });
    } else {
      createMutation.mutate({
        ...formData,
        client_secret: clientSecret,
      });
    }
  };

  const handleDelete = (id: string, providerName: string) => {
    if (window.confirm(`Delete OIDC provider "${providerName}"? This also removes linked user accounts.`)) {
      deleteMutation.mutate(id);
    }
  };

  if (isLoading) return <LoadingSkeleton />;

  return (
    <div className="settings-panel" style={{ padding: '24px', maxWidth: '900px' }}>
      {/* Added: Header with back button and add button */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <button className="btn btn--icon" title="Back" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <LogIn size={24} />
        <h2 style={{ margin: 0, flex: 1 }}>OIDC Providers</h2>
        {!showForm && (
          <button className="btn btn--primary" onClick={() => setShowForm(true)}>
            <Plus size={16} />
            Add Provider
          </button>
        )}
      </div>

      {error && (
        <div className="alert alert--error" style={{ marginBottom: '16px' }}>
          {error}
        </div>
      )}

      {/* Added: Add/Edit form */}
      {showForm && (
        <form onSubmit={handleSubmit} style={{ border: '1px solid var(--color-border)', borderRadius: '8px', padding: '16px', marginBottom: '20px' }}>
          <h3 style={{ marginTop: 0 }}>{editingId ? 'Edit Provider' : 'New OIDC Provider'}</h3>

          {/* Added: Provider identity fields */}
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-name">Provider Name</label>
            <input id="oidc-name" type="text" className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Google" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-issuer-url">Issuer URL</label>
            <input id="oidc-issuer-url" type="text" className="input" value={issuerUrl} onChange={(e) => setIssuerUrl(e.target.value)} placeholder="https://accounts.google.com" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-client-id">Client ID</label>
            <input id="oidc-client-id" type="text" className="input" value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="your-client-id.apps.googleusercontent.com" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-client-secret">Client Secret</label>
            <input id="oidc-client-secret" type="password" className="input" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} placeholder={editingId ? '(leave blank to keep current)' : 'Client secret from provider'} required={!editingId} />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-scopes">Scopes</label>
            <input id="oidc-scopes" type="text" className="input" value={scopes} onChange={(e) => setScopes(e.target.value)} placeholder="openid email profile" />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="oidc-redirect-uri">Redirect URI</label>
            <input id="oidc-redirect-uri" type="text" className="input" value={redirectUri} onChange={(e) => setRedirectUri(e.target.value)} placeholder="https://mail.example.com/api/auth/oidc/callback" required />
          </div>

          {/* Added: Display customization fields */}
          <div style={{ display: 'flex', gap: '12px', marginBottom: '12px', flexWrap: 'wrap' }}>
            <div className="form-group" style={{ flex: 1, minWidth: '180px' }}>
              <label htmlFor="oidc-icon-url">Icon URL (optional)</label>
              <input id="oidc-icon-url" type="text" className="input" value={iconUrl} onChange={(e) => setIconUrl(e.target.value)} placeholder="https://cdn.example.com/google.svg" />
            </div>
            <div className="form-group" style={{ flex: 1, minWidth: '180px' }}>
              <label htmlFor="oidc-button-label">Button Label (optional)</label>
              <input id="oidc-button-label" type="text" className="input" value={buttonLabel} onChange={(e) => setButtonLabel(e.target.value)} placeholder="Sign in with Google" />
            </div>
          </div>

          {/* Added: Auto-create toggle and default role */}
          <div style={{ display: 'flex', gap: '16px', alignItems: 'center', marginBottom: '16px', flexWrap: 'wrap' }}>
            <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={autoCreateUsers}
                onChange={(e) => setAutoCreateUsers(e.target.checked)}
              />
              Auto-create users on first login
            </label>
            <div className="form-group" style={{ minWidth: '120px' }}>
              <label htmlFor="oidc-default-role">Default Role</label>
              <input id="oidc-default-role" type="text" className="input" value={defaultRole} onChange={(e) => setDefaultRole(e.target.value)} placeholder="user" style={{ width: '120px' }} />
            </div>
          </div>

          <div style={{ display: 'flex', gap: '8px' }}>
            <button type="submit" className="btn btn--primary" disabled={createMutation.isPending || updateMutation.isPending}>
              <Save size={16} />
              {editingId ? 'Update' : 'Create'}
            </button>
            <button type="button" className="btn btn--secondary" onClick={resetForm}>
              Cancel
            </button>
          </div>
        </form>
      )}

      {/* Added: Provider list or empty state */}
      {(!providers || providers.length === 0) && !showForm ? (
        <div style={{ textAlign: 'center', padding: '40px 0', color: '#666' }}>
          <LogIn size={48} strokeWidth={1} />
          <p>No OIDC providers configured yet.</p>
          <p>Add a provider to enable social login (Google, Microsoft, etc.).</p>
        </div>
      ) : (
        <div>
          {providers?.map((provider) => (
            <div key={provider.id} style={{ border: '1px solid var(--color-border)', borderRadius: '8px', padding: '16px', marginBottom: '12px' }}>
              {/* Added: Provider summary row */}
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
                <div style={{ flex: 1, minWidth: '200px' }}>
                  <strong>{provider.name}</strong>
                  <div style={{ fontSize: '13px', color: '#666', marginTop: '2px' }}>
                    {provider.issuer_url}
                  </div>
                </div>
                <span style={{
                  padding: '2px 8px',
                  borderRadius: '12px',
                  fontSize: '12px',
                  backgroundColor: provider.active ? '#dcfce7' : '#fef2f2',
                  color: provider.active ? '#166534' : '#991b1b',
                }}>
                  {provider.active ? 'Active' : 'Inactive'}
                </span>
                {provider.auto_create_users && (
                  <span style={{ fontSize: '12px', color: '#666', padding: '2px 8px', borderRadius: '12px', backgroundColor: '#f0f0f0' }}>
                    Auto-create
                  </span>
                )}
                {/* Added: Action buttons */}
                <button
                  className="btn btn--icon"
                  title={provider.active ? 'Deactivate' : 'Activate'}
                  onClick={() => toggleActiveMutation.mutate({ id: provider.id, active: !provider.active })}
                  disabled={toggleActiveMutation.isPending}
                >
                  {provider.active ? '🔴' : '🟢'}
                </button>
                <button className="btn btn--icon" title="Edit" onClick={() => startEditing(provider)}>
                  <Save size={16} />
                </button>
                <button className="btn btn--icon" title="Delete" onClick={() => handleDelete(provider.id, provider.name)}>
                  <Trash2 size={16} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
