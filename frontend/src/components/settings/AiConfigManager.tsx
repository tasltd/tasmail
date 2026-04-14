// Added: AI configuration management UI for BYOK AI integration (TMAIL-105)
// PURPOSE: Allows users to configure their own AI API keys for email summarization and smart replies
// EXTERNAL: Uses TanStack Query for data fetching, Zustand for view state

import { useState, type FormEvent } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, ToggleLeft, ToggleRight, Zap, Brain } from 'lucide-react';
import {
  listAiConfigs,
  createAiConfig,
  updateAiConfig,
  deleteAiConfig,
  testAiConfig,
} from '../../api/ai-config';
import type { AiConfigurationResponse, AiProvider } from '../../api/ai-config';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// NOTE: Provider options with display labels and default model suggestions
const PROVIDER_OPTIONS: { value: AiProvider; label: string; defaultModels: string[] }[] = [
  { value: 'openai', label: 'OpenAI', defaultModels: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo'] },
  { value: 'anthropic', label: 'Anthropic', defaultModels: ['claude-sonnet-4-20250514', 'claude-haiku-4-20250414', 'claude-3-5-sonnet-20241022'] },
  { value: 'google', label: 'Google', defaultModels: ['gemini-2.5-pro', 'gemini-2.5-flash', 'gemini-2.0-flash'] },
  { value: 'ollama', label: 'Ollama', defaultModels: ['llama3', 'mistral', 'codellama', 'gemma2'] },
  { value: 'custom', label: 'Custom', defaultModels: [] },
];

// Added: Map provider enum to display-friendly label
function providerLabel(provider: AiProvider): string {
  const found = PROVIDER_OPTIONS.find((p) => p.value === provider);
  return found ? found.label : provider;
}

// Added: Get model suggestions for the selected provider
function getModelSuggestions(provider: AiProvider): string[] {
  const found = PROVIDER_OPTIONS.find((p) => p.value === provider);
  return found ? found.defaultModels : [];
}

export function AiConfigManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResult, setTestResult] = useState<string | null>(null);

  // Added: Form state for creating new AI configurations
  const [formProvider, setFormProvider] = useState<AiProvider>('openai');
  const [formApiKey, setFormApiKey] = useState('');
  const [formModelName, setFormModelName] = useState('gpt-4o');
  const [formBaseUrl, setFormBaseUrl] = useState('');
  const [formMaxTokens, setFormMaxTokens] = useState(500);
  const [formTemperature, setFormTemperature] = useState(0.7);

  const { data: configs, isLoading } = useQuery({
    queryKey: ['ai-configs'],
    queryFn: listAiConfigs,
  });

  const createMut = useMutation({
    mutationFn: createAiConfig,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['ai-configs'] });
      setIsCreating(false);
      resetForm();
    },
  });

  const toggleMut = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      updateAiConfig(id, { active }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['ai-configs'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteAiConfig,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['ai-configs'] }),
  });

  const testMut = useMutation({
    mutationFn: testAiConfig,
    onSuccess: (result) => {
      setTestResult(result.message);
      setTestingId(null);
    },
    onError: () => {
      setTestResult('Failed to test AI configuration');
      setTestingId(null);
    },
  });

  // Added: Reset all form fields to defaults
  function resetForm() {
    setFormProvider('openai');
    setFormApiKey('');
    setFormModelName('gpt-4o');
    setFormBaseUrl('');
    setFormMaxTokens(500);
    setFormTemperature(0.7);
  }

  // Added: Update model name when provider changes
  const handleProviderChange = (provider: AiProvider) => {
    setFormProvider(provider);
    const models = getModelSuggestions(provider);
    if (models.length > 0) {
      setFormModelName(models[0]);
    } else {
      setFormModelName('');
    }
    // Added: Clear base URL unless it's Ollama or Custom
    if (provider !== 'ollama' && provider !== 'custom') {
      setFormBaseUrl('');
    }
  };

  const handleCreate = (e: FormEvent) => {
    e.preventDefault();
    createMut.mutate({
      provider: formProvider,
      api_key: formApiKey,
      model_name: formModelName,
      base_url: formBaseUrl || undefined,
      max_tokens: formMaxTokens,
      temperature: formTemperature,
    });
  };

  const handleTest = (id: string) => {
    setTestingId(id);
    setTestResult(null);
    testMut.mutate(id);
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  // Added: Get model suggestions for the current provider
  const modelSuggestions = getModelSuggestions(formProvider);
  // Added: Show base URL field for Ollama and Custom providers
  const showBaseUrl = formProvider === 'ollama' || formProvider === 'custom';

  return (
    <div className="ai-config-manager" style={{ padding: '16px', maxWidth: '900px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>AI Configuration</h2>
        <button
          className="btn btn--primary"
          onClick={() => setIsCreating(true)}
          data-testid="add-config-btn"
        >
          <Plus size={16} /> Add Provider
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

      {/* Added: Create AI config form */}
      {isCreating && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
          data-testid="create-form"
        >
          <h3 style={{ marginBottom: '12px' }}>New AI Provider</h3>
          <form onSubmit={handleCreate}>
            <div className="composer__field">
              <label>Provider</label>
              <select
                value={formProvider}
                onChange={(e) => handleProviderChange(e.target.value as AiProvider)}
                data-testid="provider-select"
              >
                {PROVIDER_OPTIONS.map((opt) => (
                  <option key={opt.value} value={opt.value}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="composer__field">
              <label>API Key</label>
              <input
                value={formApiKey}
                onChange={(e) => setFormApiKey(e.target.value)}
                placeholder={formProvider === 'ollama' ? 'Optional for local Ollama' : 'Enter your API key'}
                type="password"
                data-testid="api-key-input"
              />
            </div>
            <div className="composer__field">
              <label>Model</label>
              <input
                value={formModelName}
                onChange={(e) => setFormModelName(e.target.value)}
                placeholder="Model name"
                required
                list="model-suggestions"
                data-testid="model-name-input"
              />
              {modelSuggestions.length > 0 && (
                <datalist id="model-suggestions">
                  {modelSuggestions.map((model) => (
                    <option key={model} value={model} />
                  ))}
                </datalist>
              )}
            </div>
            {/* Added: Base URL field for Ollama/Custom providers */}
            {showBaseUrl && (
              <div className="composer__field">
                <label>Base URL</label>
                <input
                  value={formBaseUrl}
                  onChange={(e) => setFormBaseUrl(e.target.value)}
                  placeholder={formProvider === 'ollama' ? 'http://localhost:11434' : 'https://your-api-endpoint.com/v1'}
                  type="url"
                  data-testid="base-url-input"
                />
              </div>
            )}
            {/* Added: Temperature slider */}
            <div className="composer__field">
              <label>Temperature: {formTemperature.toFixed(1)}</label>
              <input
                type="range"
                min="0"
                max="2"
                step="0.1"
                value={formTemperature}
                onChange={(e) => setFormTemperature(parseFloat(e.target.value))}
                data-testid="temperature-slider"
              />
            </div>
            {/* Added: Max tokens input */}
            <div className="composer__field">
              <label>Max Tokens</label>
              <input
                type="number"
                min="1"
                max="4096"
                value={formMaxTokens}
                onChange={(e) => setFormMaxTokens(parseInt(e.target.value, 10) || 500)}
                data-testid="max-tokens-input"
              />
            </div>
            <div className="composer__actions">
              <button type="submit" className="btn btn--primary" data-testid="create-submit">
                Save Configuration
              </button>
              <button type="button" className="btn" onClick={() => setIsCreating(false)}>
                Cancel
              </button>
            </div>
          </form>
        </div>
      )}

      {/* Added: Config list as provider cards */}
      <div style={{ marginTop: '16px' }}>
        {(!configs || configs.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No AI providers configured. Add one to enable email summarization and smart replies.
          </p>
        )}
        {configs?.map((config: AiConfigurationResponse) => (
          <div
            key={config.id}
            style={{
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
            data-testid={`config-${config.id}`}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <Brain size={18} style={{ color: 'var(--color-text-secondary)' }} />
              <div style={{ flex: 1 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  {/* Added: Provider badge */}
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: 'var(--color-primary)',
                      color: 'white',
                      fontWeight: 'bold',
                    }}
                    data-testid="provider-badge"
                  >
                    {providerLabel(config.provider)}
                  </span>
                  <span style={{ fontSize: '13px', fontWeight: 500 }}>
                    {config.model_name}
                  </span>
                  <span
                    style={{
                      fontSize: '11px',
                      padding: '1px 6px',
                      borderRadius: '10px',
                      background: config.active ? 'green' : 'gray',
                      color: 'white',
                    }}
                  >
                    {config.active ? 'Active' : 'Inactive'}
                  </span>
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', marginTop: '2px' }}>
                  Key: {config.api_key_masked} | Tokens: {config.max_tokens} | Temp: {config.temperature}
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
              {/* Added: Active/inactive toggle */}
              <button
                className="btn btn--icon"
                onClick={() =>
                  toggleMut.mutate({ id: config.id, active: !config.active })
                }
                title={config.active ? 'Deactivate' : 'Activate'}
                data-testid={`toggle-${config.id}`}
              >
                {config.active ? <ToggleRight size={20} /> : <ToggleLeft size={20} />}
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
