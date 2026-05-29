/**
 * PendingSyncBanner (TMAIL-88)
 *
 * Surfaces the offline-queue state at the top of the app shell:
 *   - When offline + queue is non-empty: amber "Offline — N actions queued".
 *   - When online + queue is non-empty: blue "Syncing N actions…" with a manual
 *     "Retry now" button that calls processPending().
 *   - When the queue is empty: renders nothing.
 *
 * No external CSS dependency: styles are inline with theme tokens so the banner
 * works in both the classic SPA and the alt-UI shell.
 */
import { useState } from 'react';
import { backgroundSync } from '../../utils/background-sync';
import { usePendingSyncCount } from '../../hooks/usePendingSyncCount';
import { useOnlineStatus } from '../../hooks/useOnlineStatus';

export function PendingSyncBanner() {
  const count = usePendingSyncCount();
  const isOnline = useOnlineStatus();
  const [busy, setBusy] = useState(false);

  if (count === 0) return null;

  const handleRetry = async () => {
    if (busy) return;
    setBusy(true);
    try {
      await backgroundSync.processPending();
    } finally {
      setBusy(false);
    }
  };

  const tone = isOnline
    ? { bg: 'var(--color-info-bg, #e7f1ff)', fg: 'var(--color-info, #0b5ed7)' }
    : { bg: 'var(--color-warning-bg, #fff3cd)', fg: 'var(--color-warning, #b08800)' };

  const label = isOnline
    ? `Syncing ${count} action${count === 1 ? '' : 's'}…`
    : `Offline — ${count} action${count === 1 ? '' : 's'} queued`;

  return (
    <div
      role="status"
      aria-live="polite"
      data-testid="pending-sync-banner"
      className="pending-sync-banner"
      style={{
        padding: '6px 12px',
        background: tone.bg,
        color: tone.fg,
        fontSize: '13px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '8px',
        borderBottom: '1px solid var(--color-border, #e0e0e0)',
      }}
    >
      <span>
        <strong>Pending sync:</strong> {label}
      </span>
      {isOnline && (
        <button
          type="button"
          onClick={handleRetry}
          disabled={busy}
          data-testid="pending-sync-retry"
          style={{
            background: 'transparent',
            border: `1px solid ${tone.fg}`,
            color: tone.fg,
            borderRadius: '4px',
            padding: '2px 10px',
            fontSize: '12px',
            cursor: busy ? 'not-allowed' : 'pointer',
            opacity: busy ? 0.6 : 1,
          }}
        >
          {busy ? 'Retrying…' : 'Retry now'}
        </button>
      )}
    </div>
  );
}
