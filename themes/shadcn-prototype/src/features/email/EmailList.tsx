import { Star, Paperclip } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import type { Email } from '@/types/ui';

interface EmailListProps {
  emails: Email[];
  selectedEmailId: string | null;
  onSelectEmail: (emailId: string) => void;
  // Added: TMAIL-315 — star button toggles the IMAP \Flagged keyword via
  // PATCH /api/folders/{folder}/messages/{uid}/flag. The container
  // (EmailClient) owns the mutation + cache invalidation; this list stays
  // presentational and just reports user intent.
  onToggleStar?: (emailId: string, currentlyStarred: boolean) => void;
}

export function EmailList({ emails, selectedEmailId, onSelectEmail, onToggleStar }: EmailListProps) {
  if (emails.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-zinc-500">
        No emails in this folder
      </div>
    );
  }

  return (
    <div className="overflow-y-auto">
      {emails.map((email) => (
        <div
          key={email.id}
          onClick={() => onSelectEmail(email.id)}
          className={`border-b border-zinc-200 dark:border-zinc-800 p-4 cursor-pointer transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-900 ${
            selectedEmailId === email.id ? 'bg-zinc-50 dark:bg-zinc-900' : ''
          } ${!email.read ? 'bg-blue-50/50 dark:bg-blue-950/20' : ''}`}
        >
          <div className="flex items-start gap-3">
            <button
              type="button"
              aria-pressed={email.starred}
              aria-label={email.starred ? `Unstar email from ${email.from}` : `Star email from ${email.from}`}
              onClick={(e) => {
                e.stopPropagation();
                onToggleStar?.(email.id, email.starred);
              }}
              className="mt-1 cursor-pointer rounded-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
            >
              <Star
                className={`size-4 ${
                  email.starred
                    ? 'fill-yellow-400 text-yellow-400'
                    : 'text-zinc-400 hover:text-yellow-400'
                }`}
              />
            </button>

            <div className="flex-1 min-w-0">
              <div className="flex items-center justify-between mb-1">
                <span className={`font-medium truncate ${!email.read ? 'font-semibold' : ''}`}>
                  {email.from}
                </span>
                <span className="text-xs text-zinc-500 ml-2 whitespace-nowrap">
                  {formatDistanceToNow(email.timestamp, { addSuffix: true })}
                </span>
              </div>

              <div className={`text-sm truncate mb-1 ${!email.read ? 'font-medium' : 'text-zinc-600 dark:text-zinc-400'}`}>
                {email.subject}
              </div>

              <div className="text-sm text-zinc-500 truncate">
                {email.preview}
              </div>

              {email.attachments && email.attachments.length > 0 && (
                <div className="flex items-center gap-1 mt-2 text-xs text-zinc-500">
                  <Paperclip className="size-3" />
                  <span>{email.attachments.length} attachment{email.attachments.length > 1 ? 's' : ''}</span>
                </div>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
