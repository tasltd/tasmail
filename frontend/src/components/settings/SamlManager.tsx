// Added: SAML 2.0 SSO configuration manager component for TMAIL-101
import { useState } from 'react';
import type { FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { KeyRound, Plus, Save, Trash2, ExternalLink, ArrowLeft } from 'lucide-react';
import {
  listSamlConfigs,
  createSamlConfig,
  updateSamlConfig,
  deleteSamlConfig,
  getSamlLoginUrl,
} from '../../api/saml';
import type {
  SamlConfiguration,
  CreateSamlConfigRequest,
} from '../../api/saml';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Standard NameID format options for the dropdown
const NAME_ID_FORMATS = [
  { value: 'urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress', label: 'Email Address' },
  { value: 'urn:oasis:names:tc:SAML:1.1:nameid-format:unspecified', label: 'Unspecified' },
  { value: 'urn:oasis:names:tc:SAML:2.0:nameid-format:persistent', label: 'Persistent' },
  { value: 'urn:oasis:names:tc:SAML:2.0:nameid-format:transient', label: 'Transient' },
];

/**
 * PURPOSE: Admin UI for managing SAML 2.0 SSO IdP configurations
 * CONSTRAINTS: Only admins should access — route protection handled by backend
 * EXTERNAL: Uses /api/admin/saml endpoints for CRUD, /api/auth/saml/:id/login for test SSO
 */
export function SamlManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [showForm, setShowForm] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [error, setError] = useState('');

  // Added: Form state for SAML config fields
  const [name, setName] = useState('');
  const [entityId, setEntityId] = useState('');
  const [ssoUrl, setSsoUrl] = useState('');
  const [sloUrl, setSloUrl] = useState('');
  const [certificate, setCertificate] = useState('');
  const [nameIdFormat, setNameIdFormat] = useState(NAME_ID_FORMATS[0].value);
  const [attributeMapping, setAttributeMapping] = useState('{"email": "email", "name": "displayName"}');

  // Added: Fetch all SAML configurations
  const { data: configs, isLoading } = useQuery<SamlConfiguration[]>({
    queryKey: ['saml-configs'],
    queryFn: listSamlConfigs,
  });

  const createMutation = useMutation({
    mutationFn: (data: CreateSamlConfigRequest) => createSamlConfig(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['saml-configs'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: Record<string, unknown> }) =>
      updateSamlConfig(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['saml-configs'] });
      resetForm();
      setError('');
    },
    onError: (err: Error) => setError(err.message),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteSamlConfig(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['saml-configs'] });
    },
    onError: (err: Error) => setError(err.message),
  });

  // Added: Reset form to initial state
  const resetForm = () => {
    setShowForm(false);
    setEditingId(null);
    setName('');
    setEntityId('');
    setSsoUrl('');
    setSloUrl('');
    setCertificate('');
    setNameIdFormat(NAME_ID_FORMATS[0].value);
    setAttributeMapping('{"email": "email", "name": "displayName"}');
  };

  // Added: Populate form for editing an existing config
  const startEditing = (config: SamlConfiguration) => {
    setEditingId(config.id);
    setShowForm(true);
    setName(config.name);
    setEntityId(config.entity_id);
    setSsoUrl(config.sso_url);
    setSloUrl(config.slo_url ?? '');
    setCertificate(config.certificate);
    setNameIdFormat(config.name_id_format);
    setAttributeMapping(JSON.stringify(config.attribute_mapping, null, 2));
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();

    // Added: Validate attribute mapping is valid JSON
    let parsedMapping: Record<string, string>;
    try {
      parsedMapping = JSON.parse(attributeMapping);
    } catch {
      setError('Attribute mapping must be valid JSON');
      return;
    }

    const formData = {
      name,
      entity_id: entityId,
      sso_url: ssoUrl,
      slo_url: sloUrl || undefined,
      certificate,
      name_id_format: nameIdFormat,
      attribute_mapping: parsedMapping,
    };

    if (editingId) {
      updateMutation.mutate({ id: editingId, data: formData });
    } else {
      createMutation.mutate(formData);
    }
  };

  const handleDelete = (id: string, configName: string) => {
    if (window.confirm(`Delete SAML configuration "${configName}"? This cannot be undone.`)) {
      deleteMutation.mutate(id);
    }
  };

  // Added: Open SAML login URL in a new tab for testing
  const handleTestSso = async (id: string) => {
    try {
      const response = await getSamlLoginUrl(id);
      window.open(response.redirect_url, '_blank');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to get SAML login URL');
    }
  };

  // Added: Toggle active state via update mutation
  const handleToggleActive = (config: SamlConfiguration) => {
    updateMutation.mutate({
      id: config.id,
      data: { active: !config.active },
    });
  };

  if (isLoading) return <LoadingSkeleton />;

  return (
    <div className="settings-panel" style={{ padding: '24px', maxWidth: '900px' }}>
      {/* Added: Header with back button and add button */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '20px' }}>
        <button className="btn btn--icon" title="Back" onClick={() => setViewMode('list')}>
          <ArrowLeft size={18} />
        </button>
        <KeyRound size={24} />
        <h2 style={{ margin: 0, flex: 1 }}>SAML Single Sign-On</h2>
        {!showForm && (
          <button className="btn btn--primary" onClick={() => setShowForm(true)}>
            <Plus size={16} />
            Add IdP
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
          <h3 style={{ marginTop: 0 }}>{editingId ? 'Edit IdP Configuration' : 'New SAML IdP Configuration'}</h3>

          {/* Added: Identity provider fields */}
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-name">Configuration Name</label>
            <input id="saml-name" type="text" className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Okta SSO" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-entity-id">IdP Entity ID</label>
            <input id="saml-entity-id" type="text" className="input" value={entityId} onChange={(e) => setEntityId(e.target.value)} placeholder="https://idp.example.com/saml/metadata" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-sso-url">SSO URL</label>
            <input id="saml-sso-url" type="url" className="input" value={ssoUrl} onChange={(e) => setSsoUrl(e.target.value)} placeholder="https://idp.example.com/sso/saml" required />
          </div>
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-slo-url">SLO URL (optional)</label>
            <input id="saml-slo-url" type="url" className="input" value={sloUrl} onChange={(e) => setSloUrl(e.target.value)} placeholder="https://idp.example.com/slo/saml" />
          </div>

          {/* Added: Certificate textarea for IdP X.509 certificate */}
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-certificate">IdP Certificate (X.509 PEM)</label>
            <textarea
              id="saml-certificate"
              className="input"
              value={certificate}
              onChange={(e) => setCertificate(e.target.value)}
              placeholder="-----BEGIN CERTIFICATE-----&#10;MIICpDCCAYwCCQ...&#10;-----END CERTIFICATE-----"
              rows={6}
              style={{ fontFamily: 'monospace', fontSize: '13px', resize: 'vertical' }}
              required
            />
          </div>

          {/* Added: NameID format dropdown */}
          <div className="form-group" style={{ marginBottom: '12px' }}>
            <label htmlFor="saml-name-id-format">Name ID Format</label>
            <select
              id="saml-name-id-format"
              className="input"
              value={nameIdFormat}
              onChange={(e) => setNameIdFormat(e.target.value)}
            >
              {NAME_ID_FORMATS.map((format) => (
                <option key={format.value} value={format.value}>
                  {format.label}
                </option>
              ))}
            </select>
          </div>

          {/* Added: Attribute mapping JSON editor */}
          <div className="form-group" style={{ marginBottom: '16px' }}>
            <label htmlFor="saml-attribute-mapping">Attribute Mapping (JSON)</label>
            <textarea
              id="saml-attribute-mapping"
              className="input"
              value={attributeMapping}
              onChange={(e) => setAttributeMapping(e.target.value)}
              rows={4}
              style={{ fontFamily: 'monospace', fontSize: '13px', resize: 'vertical' }}
            />
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
          <KeyRound size={48} strokeWidth={1} />
          <p>No SAML configurations yet.</p>
          <p>Add an Identity Provider to enable SSO for your organization.</p>
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
                    {config.entity_id}
                  </div>
                  <div style={{ fontSize: '13px', color: '#888', marginTop: '2px' }}>
                    SSO: {config.sso_url}
                  </div>
                </div>
                {/* Added: Active/inactive status badge */}
                <button
                  onClick={() => handleToggleActive(config)}
                  title={config.active ? 'Click to deactivate' : 'Click to activate'}
                  style={{
                    padding: '2px 8px',
                    borderRadius: '12px',
                    fontSize: '12px',
                    border: 'none',
                    cursor: 'pointer',
                    backgroundColor: config.active ? '#dcfce7' : '#fef2f2',
                    color: config.active ? '#166534' : '#991b1b',
                  }}
                >
                  {config.active ? 'Active' : 'Inactive'}
                </button>
                {/* Added: Test SSO button — opens IdP login in new tab */}
                <button
                  className="btn btn--icon"
                  title="Test SSO"
                  onClick={() => handleTestSso(config.id)}
                >
                  <ExternalLink size={16} />
                </button>
                <button className="btn btn--icon" title="Edit" onClick={() => startEditing(config)}>
                  <Save size={16} />
                </button>
                <button className="btn btn--icon" title="Delete" onClick={() => handleDelete(config.id, config.name)}>
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
