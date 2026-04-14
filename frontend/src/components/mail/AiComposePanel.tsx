// Added: AI Compose Panel for full draft generation (TMAIL-134)
// PURPOSE: Provides a UI for users to describe what they want to write and get an AI-generated email draft
// EXTERNAL: Uses composeEmail API from ai-config module
// CONSTRAINTS: Requires an active AI configuration; tone and length are optional

import { useState } from 'react';
import { Sparkles, RefreshCw } from 'lucide-react';
import { composeEmail, type ComposeTone, type ComposeLength } from '../../api/ai-config';

interface AiComposePanelProps {
  /** PURPOSE: Callback when user accepts the generated draft */
  onUseDraft: (subject: string, body: string) => void;
}

/**
 * PURPOSE: AI-powered email composition panel that generates full drafts from user prompts
 * CONSTRAINTS: Must have an active AI provider configured in Settings > AI Config
 */
export function AiComposePanel({ onUseDraft }: AiComposePanelProps) {
  const [prompt, setPrompt] = useState('');
  const [context, setContext] = useState('');
  const [tone, setTone] = useState<ComposeTone>('professional');
  const [length, setLength] = useState<ComposeLength>('medium');
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState('');
  // Added: Preview state for generated subject and body
  const [generatedSubject, setGeneratedSubject] = useState('');
  const [generatedBody, setGeneratedBody] = useState('');
  const [hasGenerated, setHasGenerated] = useState(false);

  const handleGenerate = async () => {
    if (!prompt.trim()) {
      setError('Please describe what you want to write');
      return;
    }

    setGenerating(true);
    setError('');

    try {
      const result = await composeEmail(
        prompt,
        context || undefined,
        tone,
        length,
      );
      setGeneratedSubject(result.subject);
      setGeneratedBody(result.body);
      setHasGenerated(true);
    } catch (compositionError) {
      setError(
        compositionError instanceof Error
          ? compositionError.message
          : 'Failed to generate email draft. Check your AI configuration.',
      );
    } finally {
      setGenerating(false);
    }
  };

  const handleUseDraft = () => {
    onUseDraft(generatedSubject, generatedBody);
  };

  return (
    <div className="ai-compose-panel" data-testid="ai-compose-panel">
      <div className="ai-compose-panel__header">
        <Sparkles size={16} />
        <span>AI Compose</span>
      </div>

      {error && (
        <div className="ai-compose-panel__error" data-testid="ai-compose-error">
          {error}
        </div>
      )}

      {/* Added: Prompt textarea for describing the desired email */}
      <div className="ai-compose-panel__field">
        <textarea
          data-testid="ai-compose-prompt"
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="Describe what you want to write..."
          rows={3}
          style={{
            width: '100%',
            padding: '8px',
            borderRadius: '4px',
            border: '1px solid var(--color-border)',
            resize: 'vertical',
            fontFamily: 'inherit',
            fontSize: '13px',
          }}
        />
      </div>

      {/* Added: Tone and length selectors */}
      <div className="ai-compose-panel__options" style={{ display: 'flex', gap: '8px', marginBottom: '8px' }}>
        <div style={{ flex: 1 }}>
          <label style={{ fontSize: '12px', color: 'var(--color-text-secondary)', display: 'block', marginBottom: '4px' }}>
            Tone
          </label>
          <select
            data-testid="ai-compose-tone"
            value={tone}
            onChange={(e) => setTone(e.target.value as ComposeTone)}
            style={{ width: '100%', padding: '6px', borderRadius: '4px', border: '1px solid var(--color-border)', fontSize: '13px' }}
          >
            <option value="professional">Professional</option>
            <option value="casual">Casual</option>
            <option value="friendly">Friendly</option>
            <option value="formal">Formal</option>
          </select>
        </div>
        <div style={{ flex: 1 }}>
          <label style={{ fontSize: '12px', color: 'var(--color-text-secondary)', display: 'block', marginBottom: '4px' }}>
            Length
          </label>
          <select
            data-testid="ai-compose-length"
            value={length}
            onChange={(e) => setLength(e.target.value as ComposeLength)}
            style={{ width: '100%', padding: '6px', borderRadius: '4px', border: '1px solid var(--color-border)', fontSize: '13px' }}
          >
            <option value="short">Short</option>
            <option value="medium">Medium</option>
            <option value="long">Long</option>
          </select>
        </div>
      </div>

      {/* Added: Optional context textarea */}
      <div className="ai-compose-panel__field" style={{ marginBottom: '8px' }}>
        <label style={{ fontSize: '12px', color: 'var(--color-text-secondary)', display: 'block', marginBottom: '4px' }}>
          Additional context (optional)
        </label>
        <textarea
          data-testid="ai-compose-context"
          value={context}
          onChange={(e) => setContext(e.target.value)}
          placeholder="Additional context..."
          rows={2}
          style={{
            width: '100%',
            padding: '8px',
            borderRadius: '4px',
            border: '1px solid var(--color-border)',
            resize: 'vertical',
            fontFamily: 'inherit',
            fontSize: '13px',
          }}
        />
      </div>

      {/* Added: Generate button */}
      <button
        className="btn btn--primary"
        data-testid="ai-compose-generate"
        onClick={handleGenerate}
        disabled={generating || !prompt.trim()}
        style={{ marginBottom: '8px', width: '100%' }}
      >
        <Sparkles size={14} />
        {generating ? 'Generating...' : 'Generate Draft'}
      </button>

      {/* Added: Preview area showing generated subject + body */}
      {hasGenerated && (
        <div
          className="ai-compose-panel__preview"
          data-testid="ai-compose-preview"
          style={{
            border: '1px solid var(--color-border)',
            borderRadius: '4px',
            padding: '12px',
            marginBottom: '8px',
            background: 'var(--color-bg-secondary, #f9f9f9)',
          }}
        >
          <div style={{ marginBottom: '8px' }}>
            <strong style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>Subject:</strong>
            <div style={{ fontSize: '14px', fontWeight: 500 }}>{generatedSubject}</div>
          </div>
          <div>
            <strong style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>Body:</strong>
            <div style={{ fontSize: '13px', whiteSpace: 'pre-wrap', lineHeight: 1.5 }}>{generatedBody}</div>
          </div>
        </div>
      )}

      {/* Added: Use Draft and Regenerate buttons */}
      {hasGenerated && (
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            className="btn btn--primary"
            data-testid="ai-compose-use-draft"
            onClick={handleUseDraft}
            style={{ flex: 1 }}
          >
            Use This Draft
          </button>
          <button
            className="btn btn--secondary"
            data-testid="ai-compose-regenerate"
            onClick={handleGenerate}
            disabled={generating}
            style={{ flex: 1 }}
          >
            <RefreshCw size={14} />
            Regenerate
          </button>
        </div>
      )}
    </div>
  );
}
