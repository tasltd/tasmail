// Added: Ollama local LLM management UI for TMAIL-102
// PURPOSE: Admin interface for Ollama server config, health status, and model management
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Trash2, Download, RefreshCw, Server, CheckCircle, XCircle } from 'lucide-react';
import {
  getOllamaConfig,
  updateOllamaConfig,
  getOllamaStatus,
  pullOllamaModel,
  deleteOllamaModel,
  listCachedModels,
  formatModelSize,
} from '../../api/ollama';
import type { OllamaModelInfo } from '../../api/ollama';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

export function OllamaManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: Form state for config editing
  const [formBaseUrl, setFormBaseUrl] = useState('');
  const [formEnabled, setFormEnabled] = useState(false);
  const [formDefaultModel, setFormDefaultModel] = useState('');
  const [formMaxContext, setFormMaxContext] = useState(4096);
  const [formGpuLayers, setFormGpuLayers] = useState(-1);
  const [configLoaded, setConfigLoaded] = useState(false);

  // Added: Pull model input state
  const [pullModelName, setPullModelName] = useState('');
  const [pullMessage, setPullMessage] = useState<string | null>(null);

  // Added: Fetch current config
  const { data: config, isLoading: configLoading } = useQuery({
    queryKey: ['ollama-config'],
    queryFn: getOllamaConfig,
  });

  // Added: Populate form when config loads
  if (config && !configLoaded) {
    setFormBaseUrl(config.base_url);
    setFormEnabled(config.enabled);
    setFormDefaultModel(config.default_model || '');
    setFormMaxContext(config.max_context_length || 4096);
    setFormGpuLayers(config.gpu_layers ?? -1);
    setConfigLoaded(true);
  }

  // Added: Fetch server status (health + models)
  const { data: status, isLoading: statusLoading, refetch: refetchStatus } = useQuery({
    queryKey: ['ollama-status'],
    queryFn: getOllamaStatus,
    // NOTE: Refetch every 30 seconds while on this page
    refetchInterval: 30000,
  });

  // Added: Fetch cached models from DB
  const { data: cachedModels } = useQuery({
    queryKey: ['ollama-cached-models'],
    queryFn: listCachedModels,
  });

  // Added: Update config mutation
  const updateMut = useMutation({
    mutationFn: updateOllamaConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ollama-config'] });
      queryClient.invalidateQueries({ queryKey: ['ollama-status'] });
    },
  });

  // Added: Pull model mutation
  const pullMut = useMutation({
    mutationFn: pullOllamaModel,
    onSuccess: (result) => {
      setPullMessage(result.success ? `Model pulled: ${result.message}` : `Pull failed: ${result.message}`);
      setPullModelName('');
      queryClient.invalidateQueries({ queryKey: ['ollama-status'] });
      queryClient.invalidateQueries({ queryKey: ['ollama-cached-models'] });
    },
    onError: () => {
      setPullMessage('Failed to pull model');
    },
  });

  // Added: Delete model mutation
  const deleteMut = useMutation({
    mutationFn: deleteOllamaModel,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ollama-status'] });
      queryClient.invalidateQueries({ queryKey: ['ollama-cached-models'] });
    },
  });

  const handleConfigSave = (e: React.FormEvent) => {
    e.preventDefault();
    updateMut.mutate({
      base_url: formBaseUrl,
      enabled: formEnabled,
      default_model: formDefaultModel || undefined,
      max_context_length: formMaxContext,
      gpu_layers: formGpuLayers,
    });
  };

  const handlePull = (e: React.FormEvent) => {
    e.preventDefault();
    if (!pullModelName.trim()) return;
    setPullMessage(null);
    pullMut.mutate(pullModelName.trim());
  };

  if (configLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="ollama-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Ollama LLM Server</h2>
        <button
          className="btn"
          onClick={() => refetchStatus()}
          title="Refresh status"
          data-testid="refresh-status"
        >
          <RefreshCw size={16} /> Refresh
        </button>
      </div>

      {/* Added: Server status dashboard */}
      <div
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
          background: 'var(--color-surface)',
        }}
        data-testid="status-panel"
      >
        <h3 style={{ marginBottom: '8px', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Server size={18} />
          Server Status
        </h3>
        {statusLoading ? (
          <p style={{ color: 'var(--color-text-secondary)' }}>Checking...</p>
        ) : (
          <div style={{ display: 'flex', gap: '24px', flexWrap: 'wrap' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
              {status?.running ? (
                <CheckCircle size={16} style={{ color: 'green' }} />
              ) : (
                <XCircle size={16} style={{ color: 'red' }} />
              )}
              <span data-testid="running-status">
                {status?.running ? 'Running' : 'Not running'}
              </span>
            </div>
            {status?.version && (
              <div>
                <span style={{ color: 'var(--color-text-secondary)' }}>Version: </span>
                <span data-testid="version">{status.version}</span>
              </div>
            )}
            <div>
              <span style={{ color: 'var(--color-text-secondary)' }}>Models: </span>
              <span data-testid="model-count">{status?.models?.length ?? 0}</span>
            </div>
          </div>
        )}
      </div>

      {/* Added: Configuration form */}
      <div
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
        }}
        data-testid="config-form"
      >
        <h3 style={{ marginBottom: '12px' }}>Configuration</h3>
        <form onSubmit={handleConfigSave}>
          <div className="composer__field">
            <label>Base URL</label>
            <input
              value={formBaseUrl}
              onChange={(e) => setFormBaseUrl(e.target.value)}
              placeholder="http://localhost:11434"
              required
              data-testid="base-url-input"
            />
          </div>
          <div className="composer__field">
            <label>Default Model</label>
            <input
              value={formDefaultModel}
              onChange={(e) => setFormDefaultModel(e.target.value)}
              placeholder="llama3.2"
              data-testid="default-model-input"
            />
          </div>
          <div style={{ display: 'flex', gap: '12px' }}>
            <div className="composer__field" style={{ flex: 1 }}>
              <label>Max Context Length</label>
              <input
                type="number"
                value={formMaxContext}
                onChange={(e) => setFormMaxContext(parseInt(e.target.value, 10) || 4096)}
                data-testid="max-context-input"
              />
            </div>
            <div className="composer__field" style={{ flex: 1 }}>
              <label>GPU Layers (-1 = auto)</label>
              <input
                type="number"
                value={formGpuLayers}
                onChange={(e) => setFormGpuLayers(parseInt(e.target.value, 10))}
                data-testid="gpu-layers-input"
              />
            </div>
          </div>
          <div className="composer__field" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <input
              type="checkbox"
              checked={formEnabled}
              onChange={(e) => setFormEnabled(e.target.checked)}
              id="ollama-enabled"
              data-testid="enabled-checkbox"
            />
            <label htmlFor="ollama-enabled">Enable Ollama as AI provider</label>
          </div>
          <div className="composer__actions">
            <button type="submit" className="btn btn--primary" data-testid="save-config-btn">
              Save Configuration
            </button>
          </div>
        </form>
      </div>

      {/* Added: Pull new model section */}
      <div
        style={{
          marginTop: '16px',
          padding: '16px',
          border: '1px solid var(--color-border)',
          borderRadius: '8px',
        }}
        data-testid="pull-section"
      >
        <h3 style={{ marginBottom: '12px' }}>Pull Model</h3>
        {pullMessage && (
          <div
            style={{
              marginBottom: '12px',
              padding: '8px 12px',
              borderRadius: '6px',
              background: 'var(--color-surface)',
              border: '1px solid var(--color-border)',
              fontSize: '13px',
            }}
            data-testid="pull-message"
          >
            {pullMessage}
          </div>
        )}
        <form onSubmit={handlePull} style={{ display: 'flex', gap: '8px' }}>
          <input
            value={pullModelName}
            onChange={(e) => setPullModelName(e.target.value)}
            placeholder="e.g., llama3.2, codellama:13b, mistral"
            style={{ flex: 1 }}
            data-testid="pull-model-input"
          />
          <button
            type="submit"
            className="btn btn--primary"
            disabled={pullMut.isPending || !pullModelName.trim()}
            data-testid="pull-model-btn"
          >
            <Download size={16} /> {pullMut.isPending ? 'Pulling...' : 'Pull'}
          </button>
        </form>
      </div>

      {/* Added: Available models list from Ollama server */}
      <div style={{ marginTop: '16px' }}>
        <h3 style={{ marginBottom: '8px' }}>Available Models</h3>
        {(!status?.models || status.models.length === 0) && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            {status?.running
              ? 'No models installed. Pull a model to get started.'
              : 'Ollama server is not running. Start Ollama and refresh.'}
          </p>
        )}
        {status?.models?.map((model: OllamaModelInfo) => (
          <div
            key={model.name}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
            }}
            data-testid={`model-${model.name}`}
          >
            <Server size={18} style={{ color: 'var(--color-text-secondary)' }} />
            <div style={{ flex: 1 }}>
              <div style={{ fontSize: '14px', fontWeight: 500 }}>{model.name}</div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                {model.parameter_size && <span>Params: {model.parameter_size} | </span>}
                {model.quantization_level && <span>Quant: {model.quantization_level} | </span>}
                <span>Size: {formatModelSize(model.size)}</span>
              </div>
            </div>
            <button
              className="btn btn--icon btn--danger"
              onClick={() => deleteMut.mutate(model.name)}
              title="Delete model"
              data-testid={`delete-${model.name}`}
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>

      {/* Added: Cached models from database */}
      {cachedModels && cachedModels.length > 0 && (
        <div style={{ marginTop: '16px' }}>
          <h3 style={{ marginBottom: '8px', color: 'var(--color-text-secondary)', fontSize: '14px' }}>
            Cached Model Metadata ({cachedModels.length})
          </h3>
          {cachedModels.map((m) => (
            <div
              key={m.id}
              style={{
                padding: '8px 12px',
                fontSize: '12px',
                color: 'var(--color-text-secondary)',
                borderBottom: '1px solid var(--color-border)',
              }}
            >
              <strong>{m.model_name}</strong>
              {m.parameter_count && <span> — {m.parameter_count}</span>}
              {m.quantization && <span> ({m.quantization})</span>}
              {m.last_pulled_at && (
                <span> — Last pulled: {new Date(m.last_pulled_at).toLocaleDateString()}</span>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
