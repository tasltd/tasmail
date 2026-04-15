// Added: Plugin management UI for extensible plugin/extension architecture (TMAIL-132)
// PURPOSE: Allows users to create, manage, test, and monitor plugins
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ChevronDown, ChevronRight, ToggleLeft, ToggleRight, Puzzle, Play } from 'lucide-react';
import {
  listPlugins,
  createPlugin,
  updatePlugin,
  deletePlugin,
  listExecutions,
  testPlugin,
} from '../../api/plugins';
import type { Plugin, PluginExecution, PluginType, PluginHook } from '../../api/plugins';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: All available plugin hook types for the checkbox form
const ALL_HOOKS: { value: PluginHook; label: string }[] = [
  { value: 'on_receive', label: 'On Receive' },
  { value: 'on_send', label: 'On Send' },
  { value: 'on_delete', label: 'On Delete' },
  { value: 'on_move', label: 'On Move' },
  { value: 'on_flag', label: 'On Flag' },
  { value: 'on_read', label: 'On Read' },
];

// NOTE: Plugin type options for the dropdown
const PLUGIN_TYPES: { value: PluginType; label: string }[] = [
  { value: 'webhook', label: 'Webhook' },
  { value: 'script', label: 'Script' },
  { value: 'filter', label: 'Filter' },
];

// Added: Execution log sub-component for a single plugin
function ExecutionLog({ pluginId }: { pluginId: string }) {
  const { data: executions, isLoading } = useQuery({
    queryKey: ['plugin-executions', pluginId],
    queryFn: () => listExecutions(pluginId),
  });

  if (isLoading) return <LoadingSkeleton rows={3} />;

  if (!executions || executions.length === 0) {
    return (
      <p style={{ color: 'var(--color-text-secondary)', fontSize: '13px', padding: '8px 0' }}>
        No executions yet.
      </p>
    );
  }

  return (
    <div style={{ marginTop: '8px' }} data-testid="execution-log">
      <table style={{ width: '100%', fontSize: '12px', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid var(--color-border)' }}>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Event</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Status</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Duration</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Time</th>
            <th style={{ textAlign: 'left', padding: '4px 8px' }}>Error</th>
          </tr>
        </thead>
        <tbody>
          {executions.map((exec: PluginExecution) => (
            <tr key={exec.id} style={{ borderBottom: '1px solid var(--color-border)' }}>
              <td style={{ padding: '4px 8px' }}>{exec.event}</td>
              <td style={{ padding: '4px 8px' }}>
                <span
                  style={{
                    color: exec.status === 'success' ? 'green' : exec.status === 'timeout' ? 'orange' : 'red',
                  }}
                >
                  {exec.status}
                </span>
              </td>
              <td style={{ padding: '4px 8px' }}>{exec.duration_ms != null ? `${exec.duration_ms}ms` : '—'}</td>
              <td style={{ padding: '4px 8px' }}>
                {new Date(exec.executed_at).toLocaleString()}
              </td>
              <td style={{ padding: '4px 8px', color: 'var(--color-text-secondary)' }}>
                {exec.error_message ?? '—'}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function PluginManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [expandedId, setExpandedId] = useState<string | null>(null);

  // Added: Form state for creating new plugins
  const [formName, setFormName] = useState('');
  const [formDescription, setFormDescription] = useState('');
  const [formType, setFormType] = useState<PluginType>('webhook');
  const [formHooks, setFormHooks] = useState<PluginHook[]>([]);
  const [formConfigUrl, setFormConfigUrl] = useState('');
  const [formConfigJson, setFormConfigJson] = useState('');

  const { data: plugins, isLoading } = useQuery({
    queryKey: ['plugins'],
    queryFn: listPlugins,
  });

  const createMut = useMutation({
    mutationFn: createPlugin,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['plugins'] });
      setIsCreating(false);
      // NOTE: Reset form for next use
      setFormName('');
      setFormDescription('');
      setFormType('webhook');
      setFormHooks([]);
      setFormConfigUrl('');
      setFormConfigJson('');
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      updatePlugin(id, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['plugins'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deletePlugin,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['plugins'] }),
  });

  const testMut = useMutation({
    mutationFn: testPlugin,
  });

  // Added: Toggle hook checkbox in the form
  const toggleHook = (hook: PluginHook) => {
    setFormHooks((prev) =>
      prev.includes(hook) ? prev.filter((h) => h !== hook) : [...prev, hook],
    );
  };

  // Added: Build config object based on plugin type
  const buildConfig = (): Record<string, unknown> => {
    if (formType === 'webhook') {
      return { url: formConfigUrl };
    }
    if (formConfigJson) {
      try {
        return JSON.parse(formConfigJson);
      } catch {
        return {};
      }
    }
    return {};
  };

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      name: formName,
      description: formDescription || undefined,
      plugin_type: formType,
      config: buildConfig(),
      hooks: formHooks,
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="plugin-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Plugins</h2>
        <button
          className="btn btn--primary"
          onClick={() => setIsCreating(true)}
        >
          <Plus size={16} /> Add Plugin
        </button>
      </div>

      {/* Added: Create plugin form */}
      {isCreating && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>New Plugin</h3>
          <form onSubmit={handleCreate}>
            <div className="composer__field">
              <label>Name</label>
              <input
                value={formName}
                onChange={(e) => setFormName(e.target.value)}
                placeholder="Plugin name"
                required
              />
            </div>
            <div className="composer__field">
              <label>Type</label>
              <select
                value={formType}
                onChange={(e) => setFormType(e.target.value as PluginType)}
                data-testid="plugin-type-select"
              >
                {PLUGIN_TYPES.map((pt) => (
                  <option key={pt.value} value={pt.value}>
                    {pt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
              <label>Hooks</label>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '4px' }}>
                {ALL_HOOKS.map((hook) => (
                  <label
                    key={hook.value}
                    style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '13px' }}
                  >
                    <input
                      type="checkbox"
                      checked={formHooks.includes(hook.value)}
                      onChange={() => toggleHook(hook.value)}
                      data-testid={`hook-${hook.value}`}
                    />
                    {hook.label}
                  </label>
                ))}
              </div>
            </div>
            {/* Added: Conditional config input based on plugin type */}
            {formType === 'webhook' ? (
              <div className="composer__field">
                <label>Webhook URL</label>
                <input
                  value={formConfigUrl}
                  onChange={(e) => setFormConfigUrl(e.target.value)}
                  placeholder="https://example.com/webhook"
                  required
                  type="url"
                />
              </div>
            ) : (
              <div className="composer__field">
                <label>Config (JSON)</label>
                <textarea
                  value={formConfigJson}
                  onChange={(e) => setFormConfigJson(e.target.value)}
                  placeholder='{"rules": []}'
                  rows={4}
                  style={{ fontFamily: 'monospace', fontSize: '13px' }}
                />
              </div>
            )}
            <div className="composer__field">
              <label>Description</label>
              <input
                value={formDescription}
                onChange={(e) => setFormDescription(e.target.value)}
                placeholder="Optional description"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" disabled={formHooks.length === 0 || !formName}>
                Create
              </button>
              <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Plugin list */}
      <div style={{ marginTop: '16px' }}>
        {(!plugins || plugins.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No plugins configured. Add one to extend TASMail with custom functionality.
          </p>
        )}
        {plugins?.map((plugin: Plugin) => (
          <div
            key={plugin.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              {/* Added: Expand/collapse toggle for execution log */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  setExpandedId(expandedId === plugin.id ? null : plugin.id)
                }
                title="Toggle executions"
              >
                {expandedId === plugin.id ? (
                  <ChevronDown size={16} />
                ) : (
                  <ChevronRight size={16} />
                )}
              </button>
              <Puzzle size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <strong style={{ fontSize: '14px' }}>{plugin.name}</strong>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: plugin.enabled ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {plugin.enabled ? 'Enabled' : 'Disabled'}
                  </span>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: 'var(--color-border)',
                      color: 'var(--color-text-secondary)',
                    }}
                  >
                    {plugin.plugin_type}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  {plugin.hooks.length} hook{plugin.hooks.length !== 1 ? 's' : ''}
                  {plugin.description && <> &middot; {plugin.description}</>}
                </div>
              </div>
              {/* Added: Test button */}
              <button
                className="btn btn--icon"
                onClick={() => testMut.mutate(plugin.id)}
                title="Test plugin"
                data-testid={`test-${plugin.id}`}
              >
                <Play size={16} />
              </button>
              {/* Added: Enable/disable toggle */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  toggleMut.mutate({ id: plugin.id, enabled: !plugin.enabled })
                }
                title={plugin.enabled ? 'Disable' : 'Enable'}
                data-testid={`toggle-${plugin.id}`}
              >
                {plugin.enabled ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
              </button>
              {/* Added: Delete button */}
              <button
                className="btn btn--icon btn--danger"
                onClick={() => deleteMut.mutate(plugin.id)}
                title="Delete"
              >
                <Trash2 size={16} />
              </button>
            </div>
            {/* Added: Expanded execution log */}
            {expandedId === plugin.id && <ExecutionLog pluginId={plugin.id} />}
          </div>
        ))}
      </div>
    </div>
  );
}
