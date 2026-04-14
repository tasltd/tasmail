// Added: Email summarization component for TMAIL-103
// PURPOSE: Inline AI-powered email and thread summarization in the message view
// EXTERNAL: Uses ai-config API (summarizeEmail, summarizeThread) for AI calls
// CONSTRAINTS: Requires an active AI configuration in Settings > AI Config

import { useState } from 'react';
import { Sparkles, X, Loader2 } from 'lucide-react';
import { useMutation } from '@tanstack/react-query';
import { summarizeEmail, summarizeThread } from '../../api/ai-config';

interface EmailSummaryProps {
  /** Current IMAP folder name */
  folder: string;
  /** UID of the currently viewed message */
  uid: number;
  /** Plain text or HTML body of the email for single-message summarization */
  emailText: string;
  /** UIDs of related thread messages (for thread summarization) */
  threadUids?: number[];
}

/**
 * PURPOSE: Renders summarize buttons and displays AI-generated summaries
 * CONSTRAINTS: Only shows thread summary button when threadUids has 2+ messages
 * EXTERNAL: Calls /api/ai/summarize and /api/ai/thread-summary endpoints
 */
export function EmailSummary({ folder, uid, emailText, threadUids }: EmailSummaryProps) {
  const [summaryText, setSummaryText] = useState<string | null>(null);
  const [summaryType, setSummaryType] = useState<'single' | 'thread' | null>(null);

  // Added: Single email summarization mutation
  const summarizeMut = useMutation({
    mutationFn: () => summarizeEmail(folder, uid, emailText),
    onSuccess: (data) => {
      setSummaryText(data.summary);
      setSummaryType('single');
    },
  });

  // Added: Thread summarization mutation
  const threadMut = useMutation({
    mutationFn: () => summarizeThread(folder, threadUids ?? []),
    onSuccess: (data) => {
      setSummaryText(data.summary);
      setSummaryType('thread');
    },
  });

  // Added: Dismiss handler clears the summary display
  const handleDismiss = () => {
    setSummaryText(null);
    setSummaryType(null);
  };

  const isLoading = summarizeMut.isPending || threadMut.isPending;
  const hasThreadMessages = threadUids && threadUids.length >= 2;
  const errorMessage = summarizeMut.error?.message || threadMut.error?.message;

  return (
    <div className="email-summary" data-testid="email-summary">
      {/* Added: Show summary card when a summary has been generated */}
      {summaryText && (
        <div className="email-summary__card" data-testid="email-summary-card">
          <div className="email-summary__card-header">
            <Sparkles size={16} data-testid="sparkles-icon" />
            <strong>{summaryType === 'thread' ? 'Thread Summary' : 'Email Summary'}</strong>
            <button
              className="btn btn--icon btn--sm email-summary__dismiss"
              onClick={handleDismiss}
              title="Dismiss summary"
              data-testid="email-summary-dismiss"
            >
              <X size={16} />
            </button>
          </div>
          <p className="email-summary__text" data-testid="email-summary-text">
            {summaryText}
          </p>
        </div>
      )}

      {/* Added: Loading indicator while AI is processing */}
      {isLoading && (
        <div className="email-summary__loading" data-testid="email-summary-loading">
          <Loader2 size={16} className="email-summary__spinner" />
          <span>Generating summary...</span>
        </div>
      )}

      {/* Added: Error display */}
      {errorMessage && !isLoading && (
        <div className="email-summary__error" data-testid="email-summary-error">
          {errorMessage}
        </div>
      )}

      {/* Added: Action buttons — only show when no summary is displayed */}
      {!summaryText && !isLoading && (
        <div className="email-summary__actions" data-testid="email-summary-actions">
          <button
            className="btn btn--sm btn--outline"
            onClick={() => summarizeMut.mutate()}
            disabled={isLoading}
            data-testid="email-summary-btn"
          >
            <Sparkles size={16} />
            Summarize
          </button>

          {hasThreadMessages && (
            <button
              className="btn btn--sm btn--outline"
              onClick={() => threadMut.mutate()}
              disabled={isLoading}
              data-testid="email-summary-thread-btn"
            >
              <Sparkles size={16} />
              Summarize Thread ({threadUids.length} messages)
            </button>
          )}
        </div>
      )}
    </div>
  );
}
