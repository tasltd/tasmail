// Added: Smart reply suggestion bar component for TMAIL-104
// PURPOSE: Inline bar in MessageView that generates AI-powered reply suggestions
// EXTERNAL: Uses getSmartReply API which calls the user's configured AI provider

import { useState } from 'react';
import { useMutation } from '@tanstack/react-query';
import { Sparkles, RefreshCw, CheckCircle } from 'lucide-react';
import { getSmartReply } from '../../api/ai-config';
import type { SmartReplyTone } from '../../api/ai-config';

interface SmartReplyBarProps {
  folder: string;
  uid: number;
  // Added: Callback to send the generated reply text to the Composer
  onUseReply: (replyText: string) => void;
}

/**
 * PURPOSE: Renders tone selection buttons, loading state, and editable reply text area
 * CONSTRAINTS: Requires an active AI config on the backend; errors surfaced inline
 */
export function SmartReplyBar({ folder, uid, onUseReply }: SmartReplyBarProps) {
  const [generatedReply, setGeneratedReply] = useState('');
  const [activeTone, setActiveTone] = useState<SmartReplyTone | null>(null);

  // Added: Mutation for generating smart reply via the AI endpoint
  const smartReplyMut = useMutation({
    mutationFn: (tone: SmartReplyTone) => getSmartReply(folder, uid, tone),
    onSuccess: (data) => {
      setGeneratedReply(data.reply);
    },
  });

  // Added: Handler that sets the active tone and triggers the mutation
  const handleToneClick = (tone: SmartReplyTone) => {
    setActiveTone(tone);
    smartReplyMut.mutate(tone);
  };

  // Added: Regenerate with the same tone
  const handleRegenerate = () => {
    if (activeTone) {
      smartReplyMut.mutate(activeTone);
    }
  };

  // Added: Send the edited reply text to the Composer via callback
  const handleUseReply = () => {
    if (generatedReply.trim()) {
      onUseReply(generatedReply);
    }
  };

  return (
    <div className="smart-reply-bar" data-testid="smart-reply-bar">
      <div className="smart-reply-bar__header">
        <Sparkles size={16} />
        <span className="smart-reply-bar__title">Smart Reply</span>
      </div>

      {/* Added: Tone selection buttons */}
      <div className="smart-reply-bar__tones" data-testid="smart-reply-tones">
        <button
          className={`btn btn--sm ${activeTone === 'brief' ? 'btn--active' : 'btn--outline'}`}
          onClick={() => handleToneClick('brief')}
          disabled={smartReplyMut.isPending}
          data-testid="smart-reply-tone-brief"
        >
          Brief
        </button>
        <button
          className={`btn btn--sm ${activeTone === 'detailed' ? 'btn--active' : 'btn--outline'}`}
          onClick={() => handleToneClick('detailed')}
          disabled={smartReplyMut.isPending}
          data-testid="smart-reply-tone-detailed"
        >
          Detailed
        </button>
        <button
          className={`btn btn--sm ${activeTone === 'decline' ? 'btn--active' : 'btn--outline'}`}
          onClick={() => handleToneClick('decline')}
          disabled={smartReplyMut.isPending}
          data-testid="smart-reply-tone-decline"
        >
          Decline
        </button>
      </div>

      {/* Added: Loading indicator while AI generates the reply */}
      {smartReplyMut.isPending && (
        <div className="smart-reply-bar__loading" data-testid="smart-reply-loading">
          <RefreshCw size={16} className="spin" />
          <span>Generating reply...</span>
        </div>
      )}

      {/* Added: Error display if smart reply generation fails */}
      {smartReplyMut.isError && (
        <div className="smart-reply-bar__error" data-testid="smart-reply-error">
          {smartReplyMut.error instanceof Error
            ? smartReplyMut.error.message
            : 'Failed to generate reply'}
        </div>
      )}

      {/* Added: Generated reply displayed in an editable text area */}
      {generatedReply && !smartReplyMut.isPending && (
        <div className="smart-reply-bar__result" data-testid="smart-reply-result">
          <textarea
            className="smart-reply-bar__textarea"
            value={generatedReply}
            onChange={(event) => setGeneratedReply(event.target.value)}
            rows={5}
            data-testid="smart-reply-textarea"
          />
          <div className="smart-reply-bar__actions">
            <button
              className="btn btn--sm btn--primary"
              onClick={handleUseReply}
              data-testid="smart-reply-use-btn"
            >
              <CheckCircle size={14} />
              Use this reply
            </button>
            <button
              className="btn btn--sm btn--outline"
              onClick={handleRegenerate}
              disabled={smartReplyMut.isPending}
              data-testid="smart-reply-regenerate-btn"
            >
              <RefreshCw size={14} />
              Regenerate
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
