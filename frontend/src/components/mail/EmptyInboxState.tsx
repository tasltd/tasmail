// Added (TMAIL-401): empty-inbox state copy. Shown when the user opens
// INBOX and there are no messages — replaces the bare "No messages"
// string with a clear hint of which address mail should be sent to.

import { Inbox } from 'lucide-react';
import { useQuery } from '@tanstack/react-query';
import { byokApi, type ImapConfig } from '../../api/byok';

// PURPOSE: pick the user's default IMAP config (or the first if none is
// flagged default). Returns null when the user has no IMAP configs yet —
// the caller falls back to a generic message in that case.
function pickDefault(configs: ImapConfig[] | undefined): ImapConfig | null {
  if (!configs || configs.length === 0) return null;
  return configs.find((c) => c.is_default) ?? configs[0];
}

interface EmptyInboxStateProps {
  // Overrides for tests — lets us assert the rendered host without
  // needing a real `useQuery` cache populated.
  defaultImapConfig?: ImapConfig | null;
}

export function EmptyInboxState({ defaultImapConfig }: EmptyInboxStateProps = {}) {
  const useDefaultProp = defaultImapConfig !== undefined;
  const { data } = useQuery({
    queryKey: ['imap-configs', 'list'],
    queryFn: byokApi.listImap,
    enabled: !useDefaultProp,
    staleTime: 60_000,
  });

  const cfg = useDefaultProp ? defaultImapConfig : pickDefault(data);
  const address = cfg ? `${cfg.username}@${cfg.host}` : null;

  return (
    <div className="empty-inbox-state" data-testid="empty-inbox-state">
      <div className="empty-inbox-state__icon" aria-hidden="true">
        <Inbox size={48} />
      </div>
      <h2 className="empty-inbox-state__title">Your inbox is empty</h2>
      {address ? (
        <p className="empty-inbox-state__sub">
          Messages sent to <code data-testid="empty-inbox-state__address">{address}</code>{' '}
          will appear here.
        </p>
      ) : (
        <p className="empty-inbox-state__sub">
          Messages sent to your configured mailbox will appear here. Add an IMAP
          server in Settings to see where mail is delivered.
        </p>
      )}
    </div>
  );
}
