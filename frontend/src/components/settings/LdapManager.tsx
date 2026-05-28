// Added: LDAP/AD directory sync manager component for TMAIL-100
import { useState } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Network, Plus, Save, Trash2, RefreshCw, ChevronDown, ChevronRight, ArrowLeft, PlugZap } from 'lucide-react';
import {
  listLdapConfigs,
  createLdapConfig,
  updateLdapConfig,
  deleteLdapConfig,
  triggerLdapSync,
  listLdapSyncLogs,
  testLdapConnection,
} from '../../api/ldap';
import type {
  LdapConfiguration,
  LdapSyncLog,
  CreateLdapConfigRequest,
} from '../../api/ldap';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Admin UI for managing LDAP/Active Directory user sync configurations
 * CONSTRAINTS: Only admins should access — route protection handled by backend
 * EXTERNAL: Uses /api/admin/ldap endpoints for CRUD + sync operations
 */
export function LdapManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [expandedLogId, setExpandedLogId] = useState<string | null>(null);
  const [error, setError] = useState('');

  // Added: Form state for LDAP config fields
  const [name, setName] = useState('');
  const [serverUrl, setServerUrl] = useState('');
  const [bindDn, setBindDn] = useState('');
  const [bindPassword, setBindPassword] = useState('');
  const [searchBase, setSearchBase] = useState('');
  const [searchFilter, setSearchFilter] = useState('(objectClass=person)');
  const [emailAttribute, setEmailAttribute] = useState('mail');
  const [nameAttribute, setNameAttribute] = useState('displayName');
  const [groupFilter, setGroupFilter] = useState('');
  const [syncInterval, setSyncInterval] = useState(60);

  // Added: Fetch all LDAP configurations
  const { data: configs, isLoading } = useQuery<LdapConfiguration[]>({
    queryKey: ['ldap-configs'],
    queryFn: listLdapConfigs,
  });

  // Added: Fetch sync logs for the expanded config
  const { data: syncLogs } = useQuery<LdapSyncLog[]>({
    queryKey: ['ldap-sync-logs', expandedLogId],
    queryFn: () => listLdapSyncLogs(expandedLogId!),
    enabled: !!expandedLogId,
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateLdapConfigRequest) => createLdapConfig(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ldap-configs'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Record<string, unknown> }) =>
      updateLdapConfig(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ldap-configs'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteLdapConfig(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ldap-configs'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  const syncMutation = useMutation({
    mutationFn: (id: string) => triggerLdapSync(id),
    onSuccess: (_, id) => {
      queryClient.invalidateQueries({ queryKey: ['ldap-configs'] });
      queryClient.invalidateQueries({ queryKey: ['ldap-sync-logs', id] });
    },
    onError: (err: Error) => setError(err.message),
  });

  // Added (TMAIL-100): "Test connection" mutation — surfaces the LDAP bind result
  // without writing anything. Success message is held in component state so the
  // admin sees a green checkmark next to the row they just tested.
  const [testedOkId, setTestedOkId] = useState<string | null>(null);
  const testMutation = useMutation({
    mutationFn: (id: string) => testLdapConnection(id),
    onSuccess: (_, id) => {
      setError('');
      setTestedOkId(id);
      // NOTE: clear the green tick after a few seconds so it doesn't linger.
      setTimeout(() => setTestedOkId((curr) => (curr === id ? null : curr)), 4000);
    },
    onError: (err: Error) => {
      setTestedOkId(null);
      setError(err.message);
    },
  });

  // Added: Reset form to initial state
  const resetForm = () => {
    setShowForm(false);
    setEditingId(null);
    setName('');
    setServerUrl('');
    setBindDn('');
    setBindPassword('');
    setSearchBase('');
    setSearchFilter('(objectClass=person)');
    setEmailAttribute('mail');
    setNameAttribute('displayName');
    setGroupFilter('');
    setSyncInterval(60);
  };

  // Added: Populate form for editing an existing config
  const startEditing = (config: LdapConfiguration) => {
    setEditingId(config.id);
    setShowForm(true);
    setName(config.name);
    setServerUrl(config.server_url);
    setBindDn(config.bind_dn);
    setBindPassword('');
    setSearchBase(config.search_base);
    setSearchFilter(config.search_filter);
    setEmailAttribute(config.email_attribute);
    setNameAttribute(config.name_attribute);
    setGroupFilter(config.group_filter ?? '');
    setSyncInterval(config.sync_interval_minutes);
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    const formData = {
      name,
      server_url: serverUrl,
      bind_dn: bindDn,
      search_base: searchBase,
      search_filter: searchFilter,
      email_attribute: emailAttribute,
      name_attribute: nameAttribute,
      group_filter: groupFilter || undefined,
      sync_interval_minutes: syncInterval,
    };

    if (editingId) {
      // Added: Only include password if user entered a new one
      updateMutation.mutate({
        id: editingId,
        data: {
          ...formData,
          ...(bindPassword ? { bind_password: bindPassword } : {}),
        },
      });
    } else {
      createMutation.mutate({
        ...formData,
        bind_password: bindPassword,
      });
    }
  };

  const handleDelete = (id: string, configName: string) => {
    if (window.confirm(`Delete LDAP configuration "${configName}"? This also removes sync history.`)) {
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
        <Network size={24} />
        <h2 style={{ margin: 0, flex: 1 }}>LDAP / Active Directory</h2>
        {!showForm && (
          <button className="btn btn--primary" onClick={() => setShowForm(true)}>
            <Plus size={16} />
            Add Configuration
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
          <h3 style={{ marginTop: 0 }}>{editingId ? 'Edit Configuration' : 'New LDAP Configuration'}</h3>

          {/* Added: Connection fields */}
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-name">Configuration Name</label>
            <input id="ldap-name" type="text" className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Corporate Active Directory" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-server-url">Server URL</label>
            <input id="ldap-server-url" type="text" className="input" value={serverUrl} onChange={(e) => setServerUrl(e.target.value)} placeholder="ldaps://ad.example.com:636" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-bind-dn">Bind DN</label>
            <input id="ldap-bind-dn" type="text" className="input" value={bindDn} onChange={(e) => setBindDn(e.target.value)} placeholder="cn=admin,dc=example,dc=com" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-bind-password">Bind Password</label>
            <input id="ldap-bind-password" type="password" className="input" value={bindPassword} onChange={(e) => setBindPassword(e.target.value)} placeholder={editingId ? '(leave blank to keep current)' : 'LDAP bind password'} required={!editingId} />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-search-base">Search Base</label>
            <input id="ldap-search-base" type="text" className="input" value={searchBase} onChange={(e) => setSearchBase(e.target.value)} placeholder="ou=Users,dc=example,dc=com" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-search-filter">Search Filter</label>
            <input id="ldap-search-filter" type="text" className="input" value={searchFilter} onChange={(e) => setSearchFilter(e.target.value)} placeholder="(objectClass=person)" />
          </div>

          {/* Added: Attribute mapping fields */}
          <div style={{ display: 'flex', gap: '12px', marginBottom: '12px', flexWrap: 'wrap' }}>
            <div className="form-group" style={{ flex: 1, minWidth: '180px' }}>
              <label htmlFor="ldap-email-attr">Email Attribute</label>
              <input id="ldap-email-attr" type="text" className="input" value={emailAttribute} onChange={(e) => setEmailAttribute(e.target.value)} placeholder="mail" />
            </div>
            <div className="form-group" style={{ flex: 1, minWidth: '180px' }}>
              <label htmlFor="ldap-name-attr">Name Attribute</label>
              <input id="ldap-name-attr" type="text" className="input" value={nameAttribute} onChange={(e) => setNameAttribute(e.target.value)} placeholder="displayName" />
            </div>
          </div>

          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="ldap-group-filter">Group Filter (optional)</label>
            <input id="ldap-group-filter" type="text" className="input" value={groupFilter} onChange={(e) => setGroupFilter(e.target.value)} placeholder="(memberOf=CN=MailUsers,OU=Groups,DC=example,DC=com)" />
          </div>
          <div className="form-group" style={{ marginBottom: '16px' }}>
            <label htmlFor="ldap-sync-interval">Sync Interval (minutes)</label>
            <input id="ldap-sync-interval" type="number" className="input" value={syncInterval} onChange={(e) => setSyncInterval(parseInt(e.target.value, 10) || 60)} min={5} max={1440} style={{ width: '120px' }} />
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

      {/* Added: Config list or empty state */}
      {(!configs || configs.length === 0) && !showForm ? (
        <div style={{ textAlign: 'center', padding: '40px 0', color: '#666' }}>
          <Network size={48} strokeWidth={1} />
          <p>No LDAP configurations yet.</p>
          <p>Add a configuration to sync users from your directory.</p>
        </div>
      ) : (
        <div>
          {configs?.map((config) => (
            <div key={config.id} style={{ border: '1px solid var(--color-border)', borderRadius: '8px', padding: '16px', marginBottom: '12px' }}>
              {/* Added: Config summary row */}
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexWrap: 'wrap' }}>
                <div style={{ flex: 1, minWidth: '200px' }}>
                  <strong>{config.name}</strong>
                  <div style={{ fontSize: '13px', color: '#666', marginTop: '2px' }}>
                    {config.server_url}
                  </div>
                </div>
                <span style={{
                  padding: '2px 8px',
                  borderRadius: '12px',
                  fontSize: '12px',
                  backgroundColor: config.active ? '#dcfce7' : '#fef2f2',
                  color: config.active ? '#166534' : '#991b1b',
                }}>
                  {config.active ? 'Active' : 'Inactive'}
                </span>
                <span style={{ fontSize: '13px', color: '#666' }}>
                  {config.users_synced ?? 0} users
                </span>
                {config.last_sync_at && (
                  <span style={{ fontSize: '13px', color: '#666' }}>
                    Last sync: {new Date(config.last_sync_at).toLocaleString()}
                  </span>
                )}
                {/* Added: Action buttons */}
                <button
                  className="btn btn--icon"
                  title="Test connection"
                  onClick={() => testMutation.mutate(config.id)}
                  disabled={testMutation.isPending}
                  data-testid={`ldap-test-${config.id}`}
                >
                  <PlugZap
                    size={16}
                    color={testedOkId === config.id ? '#16a34a' : undefined}
                  />
                </button>
                <button
                  className="btn btn--icon"
                  title="Sync now"
                  onClick={() => syncMutation.mutate(config.id)}
                  disabled={syncMutation.isPending || !config.active}
                >
                  <RefreshCw size={16} className={syncMutation.isPending ? 'spinning' : ''} />
                </button>
                <button className="btn btn--icon" title="Edit" onClick={() => startEditing(config)}>
                  <Save size={16} />
                </button>
                <button className="btn btn--icon" title="Delete" onClick={() => handleDelete(config.id, config.name)}>
                  <Trash2 size={16} />
                </button>
              </div>

              {/* Added: Expandable sync history */}
              <button
                style={{ background: 'none', border: 'none', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '4px', marginTop: '8px', fontSize: '13px', color: '#666', padding: 0 }}
                onClick={() => setExpandedLogId(expandedLogId === config.id ? null : config.id)}
              >
                {expandedLogId === config.id ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                Sync History
              </button>

              {expandedLogId === config.id && syncLogs && (
                <div style={{ marginTop: '8px' }}>
                  {syncLogs.length === 0 ? (
                    <p style={{ fontSize: '13px', color: '#666', margin: '4px 0' }}>No sync runs yet.</p>
                  ) : (
                    <table style={{ width: '100%', fontSize: '13px', borderCollapse: 'collapse' }}>
                      <thead>
                        <tr style={{ borderBottom: '1px solid var(--color-border)' }}>
                          <th style={{ textAlign: 'left', padding: '4px 8px' }}>Started</th>
                          <th style={{ textAlign: 'left', padding: '4px 8px' }}>Status</th>
                          <th style={{ textAlign: 'right', padding: '4px 8px' }}>Created</th>
                          <th style={{ textAlign: 'right', padding: '4px 8px' }}>Updated</th>
                          <th style={{ textAlign: 'right', padding: '4px 8px' }}>Disabled</th>
                          <th style={{ textAlign: 'right', padding: '4px 8px' }}>Errors</th>
                        </tr>
                      </thead>
                      <tbody>
                        {syncLogs.map((log) => (
                          <tr key={log.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
                            <td style={{ padding: '4px 8px' }}>{new Date(log.started_at).toLocaleString()}</td>
                            <td style={{ padding: '4px 8px' }}>
                              <span style={{
                                color: log.status === 'completed' ? '#166534' : log.status === 'running' ? '#854d0e' : '#991b1b',
                              }}>
                                {log.status}
                              </span>
                            </td>
                            <td style={{ textAlign: 'right', padding: '4px 8px' }}>{log.users_created}</td>
                            <td style={{ textAlign: 'right', padding: '4px 8px' }}>{log.users_updated}</td>
                            <td style={{ textAlign: 'right', padding: '4px 8px' }}>{log.users_disabled}</td>
                            <td style={{ textAlign: 'right', padding: '4px 8px' }}>
                              {Array.isArray(log.errors) && log.errors.length > 0 ? (
                                <span title={log.errors.map((e) => `${e.email}: ${e.error}`).join('\n')} style={{ color: '#991b1b', cursor: 'help' }}>
                                  {log.errors.length} error{log.errors.length !== 1 ? 's' : ''}
                                </span>
                              ) : (
                                '—'
                              )}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
