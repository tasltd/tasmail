import { useEffect, useRef } from 'react';
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
  // Added (TMAIL-325): infinite-scroll pagination. EmailClient owns the
  // useInfiniteQuery; this component only renders a sentinel <div> at the
  // bottom of the list and fires onLoadMore when the sentinel scrolls into
  // view. hasNextPage controls whether the sentinel is rendered at all so
  // we don't observe a node we'd never act on; isFetchingNextPage drives
  // the inline "Loading more…" indicator so the user gets feedback while
  // the IMAP fetch is in flight.
  hasNextPage?: boolean;
  isFetchingNextPage?: boolean;
  onLoadMore?: () => void;
}

export function EmailList({
  emails,
  selectedEmailId,
  onSelectEmail,
  onToggleStar,
  hasNextPage,
  isFetchingNextPage,
  onLoadMore,
}: EmailListProps) {
  // Added (TMAIL-325): IntersectionObserver-driven auto-pagination. The
  // sentinel is the last <div> in the scroll container; the observer fires
  // onLoadMore as soon as it enters the viewport, so the user never has to
  // click a "Load more" button. Threshold 0 + a 200px rootMargin starts the
  // next fetch just before the user reaches the bottom — keeps the perceived
  // scroll continuous rather than stop-start. The observer only attaches
  // when hasNextPage is true so we don't spin up a useless observer at the
  // bottom of fully-loaded folders.
  const sentinelRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!hasNextPage || !onLoadMore) return;
    const node = sentinelRef.current;
    if (!node) return;
    // SSR / older Firefox guard — IntersectionObserver is widely supported
    // in every browser TASMail targets, but treat its absence as a no-op
    // rather than throw so unit tests in non-DOM jsdom builds don't crash.
    if (typeof IntersectionObserver === 'undefined') return;

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          // isFetchingNextPage is intentionally NOT read here — react-query
          // de-dupes concurrent fetchNextPage() calls itself, and reading
          // the flag from a closure would create a stale-flag race when the
          // observer fires faster than the next render.
          if (entry.isIntersecting) {
            onLoadMore();
            break;
          }
        }
      },
      { rootMargin: '200px 0px 200px 0px', threshold: 0 },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [hasNextPage, onLoadMore]);

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

      {/* TMAIL-325: pagination sentinel + inline loader. The sentinel only
          renders while there's another page to fetch — once the inbox is
          fully loaded it disappears so the observer cleanup runs and we
          stop watching. data-testid keeps the unit test selector stable. */}
      {hasNextPage && (
        <div
          ref={sentinelRef}
          data-testid="email-list-sentinel"
          className="h-px"
          aria-hidden="true"
        />
      )}
      {isFetchingNextPage && (
        <div
          data-testid="email-list-loading-more"
          className="p-3 text-center text-xs text-zinc-500"
          role="status"
          aria-live="polite"
        >
          Loading more…
        </div>
      )}
    </div>
  );
}
