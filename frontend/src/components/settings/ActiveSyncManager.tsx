// Added: ActiveSync device management UI for TMAIL-130
// PURPOSE: Allows users to manage ActiveSync device registrations and admins to manage sync policies
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Smartphone, ShieldBan, ShieldCheck, Trash2, Plus, AlertTriangle } from 'lucide-react';
import {
  listDevices,
  blockDevice,
  allowDevice,
  wipeDevice,
  deleteDevice,
  listPolicies,
  createPolicy,
  updatePolicy,
  deletePolicy,
} from '../../api/activesync';
import type { CreatePolicyRequest, UpdatePolicyRequest } from '../../api/activesync';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Status badge color mapping for device statuses
function statusBadgeClass(status: string): string {
  switch (status) {
    case 'allowed':
      return 'badge badge--success';
    case 'blocked':
      return 'badge badge--danger';
    case 'pending':
      return 'badge badge--warning';
    case 'wiped':
      return 'badge badge--info';
    default:
      return 'badge';
  }
}

// Added: Device type icon label for display
function deviceTypeLabel(deviceType: string): string {
  switch (deviceType.toLowerCase()) {
    case 'iphone':
      return '📱 iPhone';
    case 'android':
      return '📱 Android';
    case 'windowsmail':
      return '💻 Windows Mail';
    case 'ipad':
      return '📱 iPad';
    default:
      return `📱 ${deviceType}`;
  }
}

export function ActiveSyncManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [activeTab, setActiveTab] = useState<'devices' | 'policies'>('devices');
  const [error, setError] = useState<string | null>(null);
  const [showPolicyForm, setShowPolicyForm] = useState(false);
  const [editingPolicyId, setEditingPolicyId] = useState<string | null>(null);

  // Added: Policy form state
  const [policyName, setPolicyName] = useState('');
  const [requireEncryption, setRequireEncryption] = useState(true);
  const [maxInactivityLockMins, setMaxInactivityLockMins] = useState('5');
  const [minPasswordLength, setMinPasswordLength] = useState('4');
  const [allowSimplePassword, setAllowSimplePassword] = useState(false);
  const [maxFailedPasswordAttempts, setMaxFailedPasswordAttempts] = useState('10');
  const [isDefault, setIsDefault] = useState(false);

  // --- Device queries ---
  const { data: devices, isLoading: devicesLoading } = useQuery({
    queryKey: ['activesync-devices'],
    queryFn: listDevices,
  });

  // --- Policy queries ---
  const { data: policies, isLoading: policiesLoading } = useQuery({
    queryKey: ['activesync-policies'],
    queryFn: listPolicies,
  });

  // --- Device mutations ---
  const blockMutation = useMutation({
    mutationFn: (id: string) => blockDevice(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-devices'] });
      setError(null);
    },
    onError: () => setError('Failed to block device'),
  });

  const allowMutation = useMutation({
    mutationFn: (id: string) => allowDevice(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-devices'] });
      setError(null);
    },
    onError: () => setError('Failed to allow device'),
  });

  const wipeMutation = useMutation({
    mutationFn: (id: string) => wipeDevice(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-devices'] });
      setError(null);
    },
    onError: () => setError('Failed to wipe device'),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteDevice(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-devices'] });
      setError(null);
    },
    onError: () => setError('Failed to delete device'),
  });

  // --- Policy mutations ---
  const createPolicyMutation = useMutation({
    mutationFn: (data: CreatePolicyRequest) => createPolicy(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-policies'] });
      resetPolicyForm();
      setError(null);
    },
    onError: () => setError('Failed to create policy'),
  });

  const updatePolicyMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdatePolicyRequest }) => updatePolicy(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-policies'] });
      resetPolicyForm();
      setError(null);
    },
    onError: () => setError('Failed to update policy'),
  });

  const deletePolicyMutation = useMutation({
    mutationFn: (id: string) => deletePolicy(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['activesync-policies'] });
      setError(null);
    },
    onError: () => setError('Failed to delete policy'),
  });

  // Added: Reset form state to defaults
  function resetPolicyForm() {
    setShowPolicyForm(false);
    setEditingPolicyId(null);
    setPolicyName('');
    setRequireEncryption(true);
    setMaxInactivityLockMins('5');
    setMinPasswordLength('4');
    setAllowSimplePassword(false);
    setMaxFailedPasswordAttempts('10');
    setIsDefault(false);
  }

  // Added: Populate form for editing an existing policy
  function startEditPolicy(policy: { id: string; name: string; require_encryption: boolean; max_inactivity_lock_mins: number | null; min_password_length: number | null; allow_simple_password: boolean; max_failed_password_attempts: number | null; is_default: boolean }) {
    setEditingPolicyId(policy.id);
    setPolicyName(policy.name);
    setRequireEncryption(policy.require_encryption);
    setMaxInactivityLockMins(policy.max_inactivity_lock_mins != null ? String(policy.max_inactivity_lock_mins) : '');
    setMinPasswordLength(policy.min_password_length != null ? String(policy.min_password_length) : '');
    setAllowSimplePassword(policy.allow_simple_password);
    setMaxFailedPasswordAttempts(policy.max_failed_password_attempts != null ? String(policy.max_failed_password_attempts) : '');
    setIsDefault(policy.is_default);
    setShowPolicyForm(true);
  }

  // Added: Handle policy form submission (create or update)
  function handlePolicySubmit(e: FormEvent) {
    e.preventDefault();
    const data = {
      name: policyName.trim(),
      require_encryption: requireEncryption,
      max_inactivity_lock_mins: maxInactivityLockMins ? parseInt(maxInactivityLockMins, 10) : null,
      min_password_length: minPasswordLength ? parseInt(minPasswordLength, 10) : null,
      allow_simple_password: allowSimplePassword,
      max_failed_password_attempts: maxFailedPasswordAttempts ? parseInt(maxFailedPasswordAttempts, 10) : null,
      is_default: isDefault,
    };

    if (editingPolicyId) {
      updatePolicyMutation.mutate({ id: editingPolicyId, data });
    } else {
      createPolicyMutation.mutate(data);
    }
  }

  if (devicesLoading || policiesLoading) {
    return <LoadingSkeleton />;
  }

  return (
    <div className="settings-panel" data-testid="activesync-manager">
      <div className="settings-panel__header">
        <button className="btn btn--ghost" onClick={() => setViewMode('list')} data-testid="back-btn">
          <ArrowLeft size={18} />
        </button>
        <h2>ActiveSync Devices</h2>
      </div>

      {error && (
        <div className="alert alert--error" data-testid="error-message">
          {error}
        </div>
      )}

      {/* Added: Tab switcher for Devices / Policies */}
      <div className="tabs" data-testid="tab-switcher">
        <button
          className={`tab ${activeTab === 'devices' ? 'tab--active' : ''}`}
          onClick={() => setActiveTab('devices')}
          data-testid="devices-tab"
        >
          Devices
        </button>
        <button
          className={`tab ${activeTab === 'policies' ? 'tab--active' : ''}`}
          onClick={() => setActiveTab('policies')}
          data-testid="policies-tab"
        >
          Policies
        </button>
      </div>

      {/* --- Devices Tab --- */}
      {activeTab === 'devices' && (
        <div data-testid="devices-panel">
          {(!devices || devices.length === 0) ? (
            <p className="empty-state" data-testid="no-devices-message">
              No ActiveSync devices registered. Devices will appear here when they connect via ActiveSync.
            </p>
          ) : (
            <div className="device-list" data-testid="device-list">
              {devices.map((device) => (
                <div key={device.id} className="card" data-testid={`device-${device.id}`}>
                  <div className="card__header">
                    <div className="card__title">
                      <Smartphone size={18} />
                      <span>{deviceTypeLabel(device.device_type)}</span>
                      <span className={statusBadgeClass(device.status)} data-testid={`status-${device.id}`}>
                        {device.status}
                      </span>
                    </div>
                  </div>
                  <div className="card__body">
                    <p><strong>Device ID:</strong> {device.device_id}</p>
                    {device.device_name && <p><strong>Name:</strong> {device.device_name}</p>}
                    {device.device_os && <p><strong>OS:</strong> {device.device_os}</p>}
                    {device.last_sync_at && (
                      <p><strong>Last Sync:</strong> {new Date(device.last_sync_at).toLocaleString()}</p>
                    )}
                  </div>
                  <div className="card__actions">
                    {device.status !== 'blocked' && (
                      <button
                        className="btn btn--sm btn--warning"
                        onClick={() => blockMutation.mutate(device.id)}
                        data-testid={`block-${device.id}`}
                      >
                        <ShieldBan size={14} /> Block
                      </button>
                    )}
                    {device.status !== 'allowed' && (
                      <button
                        className="btn btn--sm btn--success"
                        onClick={() => allowMutation.mutate(device.id)}
                        data-testid={`allow-${device.id}`}
                      >
                        <ShieldCheck size={14} /> Allow
                      </button>
                    )}
                    {device.status !== 'wiped' && (
                      <button
                        className="btn btn--sm btn--danger"
                        onClick={() => wipeMutation.mutate(device.id)}
                        data-testid={`wipe-${device.id}`}
                      >
                        <AlertTriangle size={14} /> Wipe
                      </button>
                    )}
                    <button
                      className="btn btn--sm btn--ghost"
                      onClick={() => deleteMutation.mutate(device.id)}
                      data-testid={`delete-${device.id}`}
                    >
                      <Trash2 size={14} /> Remove
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* --- Policies Tab --- */}
      {activeTab === 'policies' && (
        <div data-testid="policies-panel">
          {!showPolicyForm && (
            <button
              className="btn btn--primary"
              onClick={() => setShowPolicyForm(true)}
              data-testid="add-policy-btn"
            >
              <Plus size={18} /> Add Policy
            </button>
          )}

          {showPolicyForm && (
            <form onSubmit={handlePolicySubmit} className="form" data-testid="policy-form">
              <h3>{editingPolicyId ? 'Edit Policy' : 'New Policy'}</h3>
              <div className="form-group">
                <label htmlFor="policy-name">Policy Name</label>
                <input
                  id="policy-name"
                  type="text"
                  value={policyName}
                  onChange={(e) => setPolicyName(e.target.value)}
                  required
                  data-testid="policy-name-input"
                />
              </div>
              <div className="form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={requireEncryption}
                    onChange={(e) => setRequireEncryption(e.target.checked)}
                    data-testid="require-encryption-toggle"
                  />
                  Require Device Encryption
                </label>
              </div>
              <div className="form-group">
                <label htmlFor="max-inactivity">Max Inactivity Lock (minutes)</label>
                <input
                  id="max-inactivity"
                  type="number"
                  value={maxInactivityLockMins}
                  onChange={(e) => setMaxInactivityLockMins(e.target.value)}
                  min="1"
                  data-testid="max-inactivity-input"
                />
              </div>
              <div className="form-group">
                <label htmlFor="min-password">Min Password Length</label>
                <input
                  id="min-password"
                  type="number"
                  value={minPasswordLength}
                  onChange={(e) => setMinPasswordLength(e.target.value)}
                  min="1"
                  data-testid="min-password-input"
                />
              </div>
              <div className="form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={allowSimplePassword}
                    onChange={(e) => setAllowSimplePassword(e.target.checked)}
                    data-testid="allow-simple-password-toggle"
                  />
                  Allow Simple Passwords
                </label>
              </div>
              <div className="form-group">
                <label htmlFor="max-failed">Max Failed Password Attempts</label>
                <input
                  id="max-failed"
                  type="number"
                  value={maxFailedPasswordAttempts}
                  onChange={(e) => setMaxFailedPasswordAttempts(e.target.value)}
                  min="1"
                  data-testid="max-failed-input"
                />
              </div>
              <div className="form-group">
                <label>
                  <input
                    type="checkbox"
                    checked={isDefault}
                    onChange={(e) => setIsDefault(e.target.checked)}
                    data-testid="is-default-toggle"
                  />
                  Set as Default Policy
                </label>
              </div>
              <div className="form-actions">
                <button type="submit" className="btn btn--primary" data-testid="save-policy-btn">
                  {editingPolicyId ? 'Update' : 'Create'}
                </button>
                <button type="button" className="btn btn--ghost" onClick={resetPolicyForm} data-testid="cancel-policy-btn">
                  Cancel
                </button>
              </div>
            </form>
          )}

          {(!policies || policies.length === 0) && !showPolicyForm && (
            <p className="empty-state" data-testid="no-policies-message">
              No ActiveSync policies configured. Create a policy to enforce security requirements on connected devices.
            </p>
          )}

          {policies && policies.length > 0 && (
            <div className="policy-list" data-testid="policy-list">
              {policies.map((policy) => (
                <div key={policy.id} className="card" data-testid={`policy-${policy.id}`}>
                  <div className="card__header">
                    <div className="card__title">
                      <span>{policy.name}</span>
                      {policy.is_default && (
                        <span className="badge badge--primary" data-testid={`default-${policy.id}`}>Default</span>
                      )}
                    </div>
                  </div>
                  <div className="card__body">
                    <p><strong>Encryption:</strong> {policy.require_encryption ? 'Required' : 'Not required'}</p>
                    <p><strong>Inactivity Lock:</strong> {policy.max_inactivity_lock_mins ?? 'None'} min</p>
                    <p><strong>Min Password:</strong> {policy.min_password_length ?? 'None'} chars</p>
                    <p><strong>Simple Passwords:</strong> {policy.allow_simple_password ? 'Allowed' : 'Not allowed'}</p>
                    <p><strong>Max Failed Attempts:</strong> {policy.max_failed_password_attempts ?? 'None'}</p>
                  </div>
                  <div className="card__actions">
                    <button
                      className="btn btn--sm btn--ghost"
                      onClick={() => startEditPolicy(policy)}
                      data-testid={`edit-policy-${policy.id}`}
                    >
                      Edit
                    </button>
                    <button
                      className="btn btn--sm btn--danger"
                      onClick={() => deletePolicyMutation.mutate(policy.id)}
                      data-testid={`delete-policy-${policy.id}`}
                    >
                      <Trash2 size={14} /> Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
