// TMAIL-218: hydrate the full message body from /api/folders/{folder}/messages/{uid}
// when a row is selected. Fall back to the envelope summary while the body is
// loading. HTML body is sanitized with DOMPurify (same contract the production
// SPA uses) before insertion via the React-escape hatch — required because the
// IMAP source HTML is the actual rendering target.
import { useQuery } from '@tanstack/react-query';
import DOMPurify from 'dompurify';
import { Reply, ReplyAll, Forward, Trash2, Archive, Star, Download } from 'lucide-react';
import { format } from 'date-fns';
import { Button } from '@/components/ui/button';
import { Avatar, AvatarFallback } from '@/components/ui/avatar';
import { fetchMessage } from '@/api/messages';
import type { Email } from '@/types/ui';

interface EmailReaderProps {
  folder: string;
  uid: number | null;
  listItem: Email | null;
  onCompose: () => void;
  // Added: TMAIL-316 — star button in the reader header toggles the IMAP
  // \Flagged keyword via the same /flag endpoint the EmailList star uses.
  // EmailClient owns the mutation + cache invalidation; the reader stays
  // presentational and just reports user intent (mirrors EmailList — TMAIL-315).
  onToggleStar?: (uid: number, currentlyStarred: boolean) => void;
  // Added: TMAIL-317 — Archive button moves the message to the IMAP "Archive"
  // folder via the same /move endpoint MessageView uses in the classic SPA.
  // EmailClient owns the mutation, optimistic update, and cache invalidation;
  // the reader stays presentational.
  onArchive?: (uid: number) => void;
  // Added: TMAIL-318 — Delete button. From any non-trash folder the backend
  // /delete handler soft-deletes by moving to the per-user trash folder
  // (Stalwart "Deleted Items", Dovecot "Trash", Gmail "[Gmail]/Trash" — see
  // imap_service.rs::trash_folder()). From the trash folder itself it is a
  // permanent EXPUNGE — EmailClient gates that path behind window.confirm()
  // before invoking the mutation. The reader stays presentational and only
  // reports user intent; `isPermanentDelete` lets the aria-label spell out
  // the consequence to screen-reader users.
  onDelete?: (uid: number) => void;
  isPermanentDelete?: boolean;
}

export function EmailReader({
  folder,
  uid,
  listItem,
  onCompose,
  onToggleStar,
  onArchive,
  onDelete,
  isPermanentDelete,
}: EmailReaderProps) {
  const messageQuery = useQuery({
    queryKey: ['message', folder, uid],
    queryFn: () => fetchMessage(folder, uid!),
    enabled: uid != null,
  });

  if (uid == null) {
    return (
      <div className="flex items-center justify-center h-full text-zinc-500">
        Select an email to read
      </div>
    );
  }

  const m = messageQuery.data;
  const subject = m?.subject ?? listItem?.subject ?? '(loading…)';
  const from = m?.from ?? listItem?.from ?? '(unknown)';
  const fromEmail = m?.from ?? listItem?.fromEmail ?? '';
  const to = m?.to ?? '';
  const dateRaw = m?.date ?? listItem?.timestamp ?? new Date();
  const timestamp = typeof dateRaw === 'string' ? new Date(dateRaw) : dateRaw;
  const isStarred = m?.flags?.some((f: string) => f.includes('Flagged')) ?? listItem?.starred ?? false;
  const htmlBody = m?.html_body ?? '';
  const textBody = m?.text_body ?? '';
  const attachments = m?.attachments ?? [];

  const initials = from
    .split(' ')
    .map((n: string) => n[0])
    .join('')
    .toUpperCase()
    .slice(0, 2);

  // Sanitize once so the JSX below stays readable.
  const safeHtml = htmlBody ? DOMPurify.sanitize(htmlBody) : '';

  return (
    <div className="flex flex-col h-full">
      <div className="border-b border-zinc-200 dark:border-zinc-800 p-4">
        <div className="flex items-start justify-between mb-4">
          <h2 className="text-2xl font-semibold flex-1">{subject}</h2>
          {/* Added: TMAIL-316 — wired to /flag via EmailClient's mutation.
              Native <button> + aria-pressed so screen readers announce the
              toggle state, matching the EmailList pattern (TMAIL-315). */}
          <button
            type="button"
            aria-pressed={isStarred}
            aria-label={isStarred ? `Unstar email from ${from}` : `Star email from ${from}`}
            disabled={uid == null || !onToggleStar}
            onClick={() => {
              if (uid != null) onToggleStar?.(uid, isStarred);
            }}
            className="inline-flex items-center justify-center size-9 rounded-md hover:bg-zinc-100 dark:hover:bg-zinc-800 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Star
              className={`size-5 ${isStarred ? 'fill-yellow-400 text-yellow-400' : 'text-zinc-400 hover:text-yellow-400'}`}
            />
          </button>
        </div>

        <div className="flex items-center gap-3 mb-4">
          <Avatar>
            <AvatarFallback className="bg-gradient-to-br from-blue-500 to-purple-600 text-white">{initials}</AvatarFallback>
          </Avatar>
          <div className="flex-1 min-w-0">
            <div className="font-medium truncate">{from}</div>
            <div className="text-sm text-zinc-500 truncate">
              {fromEmail}
              {to ? ` → ${to}` : ''}
            </div>
          </div>
          <div className="text-xs sm:text-sm text-zinc-500 shrink-0 ml-2">
            <span className="hidden sm:inline">{format(timestamp, 'MMM d, yyyy • h:mm a')}</span>
            <span className="sm:hidden">{format(timestamp, 'MMM d')}</span>
          </div>
        </div>

        <div className="flex items-center gap-1 sm:gap-2 flex-wrap">
          <Button variant="outline" size="sm" onClick={onCompose}>
            <Reply className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply</span>
          </Button>
          <Button variant="outline" size="sm" onClick={onCompose}>
            <ReplyAll className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Reply All</span>
          </Button>
          <Button variant="outline" size="sm" onClick={onCompose}>
            <Forward className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Forward</span>
          </Button>
          <div className="flex-1" />
          {/* Added: TMAIL-317 — wires to /move via EmailClient's archiveMutation
              targeting the IMAP "Archive" folder (auto-created on first use). */}
          <Button
            variant="outline"
            size="sm"
            disabled={uid == null || !onArchive}
            aria-label={`Archive email from ${from}`}
            onClick={() => {
              if (uid != null) onArchive?.(uid);
            }}
          >
            <Archive className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Archive</span>
          </Button>
          {/* Added: TMAIL-318 — wires to /delete via EmailClient's deleteMutation.
              Backend resolves the per-user trash folder name and either moves
              the message there (soft delete) or EXPUNGEs it permanently if the
              active folder IS that trash folder. EmailClient gates permanent
              delete behind window.confirm() before firing the mutation. */}
          <Button
            variant="outline"
            size="sm"
            disabled={uid == null || !onDelete}
            aria-label={
              isPermanentDelete
                ? `Permanently delete email from ${from}`
                : `Delete email from ${from}`
            }
            onClick={() => {
              if (uid != null) onDelete?.(uid);
            }}
          >
            <Trash2 className="size-4 sm:mr-2" />
            <span className="hidden sm:inline">Delete</span>
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {messageQuery.isLoading && (
          <div className="text-zinc-500 text-sm">Loading message…</div>
        )}
        {messageQuery.isError && (
          <div className="text-red-600 text-sm">
            Couldn't load message: {String(messageQuery.error)}
          </div>
        )}
        {!messageQuery.isLoading && !messageQuery.isError && (
          <>
            {safeHtml ? (
              // eslint-disable-next-line react/no-danger -- DOMPurify-sanitized above
              <div className="prose dark:prose-invert max-w-none" dangerouslySetInnerHTML={{ __html: safeHtml }} />
            ) : (
              <pre className="whitespace-pre-wrap font-sans text-sm">{textBody || '(empty body)'}</pre>
            )}

            {attachments.length > 0 && (
              <div className="mt-8 space-y-2">
                <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300 mb-3">
                  Attachments ({attachments.length})
                </h3>
                {attachments.map((attachment, index) => (
                  <div
                    key={index}
                    className="flex items-center justify-between p-3 border border-zinc-200 dark:border-zinc-800 rounded-lg bg-zinc-50 dark:bg-zinc-900"
                  >
                    <div className="flex items-center gap-3">
                      <div className="size-10 rounded bg-blue-100 dark:bg-blue-900/30 flex items-center justify-center">
                        <Download className="size-5 text-blue-600 dark:text-blue-400" />
                      </div>
                      <div>
                        <div className="font-medium text-sm">{attachment.filename ?? '(unnamed)'}</div>
                        <div className="text-xs text-zinc-500">
                          {attachment.size != null ? `${Math.round((attachment.size as number) / 1024)} KB` : ''}
                        </div>
                      </div>
                    </div>
                    <Button variant="outline" size="sm">
                      <Download className="size-4 mr-2" />
                      Download
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
