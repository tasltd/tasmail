import { useEffect, useMemo, useRef, useState } from 'react';
import { Star, Paperclip, ChevronDown, ChevronRight, MessageSquare } from 'lucide-react';
import { formatDistanceToNow } from 'date-fns';
import { Checkbox } from '@/components/ui/checkbox';
import type { Email } from '@/types/ui';
import { formatParticipants, groupByThread, type EmailThread } from './threadGrouping';

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
  // Added (TMAIL-326): multi-select. EmailClient owns the Set<string> of
  // selected ids; this list only renders the per-row Checkbox and forwards
  // user intent. `onToggleSelect` carries the modifier-key flag so the
  // container can route to range-select (shift) vs single-toggle (plain).
  // When `selectedIds` is undefined the list renders without checkboxes —
  // keeps the prop optional so legacy callers (none today, but the alt-UI
  // EmailReader walkthrough specs treat the list as a presentational shell)
  // don't have to opt in.
  selectedIds?: ReadonlySet<string>;
  onToggleSelect?: (emailId: string, shiftKey: boolean) => void;
  // Added (TMAIL-350): when true, the list renders conversation-grouped
  // rows via groupByThread() — collapsed thread headers + expand-on-click
  // detail rows. EmailClient owns the toggle (per-folder, persisted in
  // localStorage); the list stays presentational and just routes between
  // the threaded and flat renderers based on this flag.
  threaded?: boolean;
}

export function EmailList({
  emails,
  selectedEmailId,
  onSelectEmail,
  onToggleStar,
  hasNextPage,
  isFetchingNextPage,
  onLoadMore,
  selectedIds,
  onToggleSelect,
  threaded = false,
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

  // Added (TMAIL-350): bucket the flat envelope list into conversations
  // whenever threading is on. Memoised on the input array identity so a
  // re-render of the parent (e.g. star toggle) doesn't re-run the union-find
  // for an unchanged list.
  const threads: EmailThread[] = useMemo(
    () => (threaded ? groupByThread(emails) : []),
    [threaded, emails],
  );

  // Added (TMAIL-350): per-thread expand/collapse state. Default collapsed —
  // Gmail behaviour. Auto-expand a thread when its latest message is the
  // currently selected one (so opening a reply from another surface like a
  // notification leaves the thread visibly open).
  const [expandedThreadIds, setExpandedThreadIds] = useState<Set<string>>(() => new Set());
  useEffect(() => {
    if (!threaded || !selectedEmailId) return;
    // Find which thread contains the selected message and expand it.
    for (const t of threads) {
      if (t.messages.some((m) => m.id === selectedEmailId)) {
        setExpandedThreadIds((prev) => {
          if (prev.has(t.id)) return prev;
          const next = new Set(prev);
          next.add(t.id);
          return next;
        });
        break;
      }
    }
  }, [threaded, selectedEmailId, threads]);

  const toggleThreadExpansion = (threadId: string) => {
    setExpandedThreadIds((prev) => {
      const next = new Set(prev);
      if (next.has(threadId)) next.delete(threadId);
      else next.add(threadId);
      return next;
    });
  };

  if (emails.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-zinc-500">
        No emails in this folder
      </div>
    );
  }

  // Added (TMAIL-326): show checkbox column only when the container has
  // opted into multi-select. Keeps the rendered DOM lean for callers that
  // don't need it and lets the list keep its old visual density.
  const multiSelectEnabled = !!onToggleSelect;

  // ── Threaded view (TMAIL-350) ──────────────────────────────────────────
  // Renders one row per conversation. Collapsed by default; clicking the
  // chevron OR the row body toggles expansion. When expanded, each message
  // in the thread is rendered as an indented child row so the user can
  // click into any single reply. Thread-of-one buckets render identically
  // to the flat row (no chevron, no participant badge) so the user doesn't
  // see redundant chrome on solo messages.
  if (threaded) {
    return (
      <div className="overflow-y-auto" data-testid="email-list-threaded">
        {threads.map((thread) => {
          const isExpanded = expandedThreadIds.has(thread.id);
          const isSolo = thread.messages.length === 1;
          const latest = thread.messages[0];
          const isLatestChecked = selectedIds?.has(latest.id) ?? false;
          const participantsLabel = formatParticipants(thread.participants);

          // For a thread-of-one we render the same row shape as the flat
          // view so the visual baseline is preserved. The only difference
          // is the click target routes through the thread expansion no-op
          // so the row behaves identically to the flat case.
          return (
            <div key={thread.id} data-testid={`email-thread-${thread.id}`}>
              <div
                onClick={() => {
                  if (!isSolo) {
                    toggleThreadExpansion(thread.id);
                  }
                  onSelectEmail(latest.id);
                }}
                className={`border-b border-zinc-200 dark:border-zinc-800 p-4 cursor-pointer transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-900 ${
                  selectedEmailId === latest.id ? 'bg-zinc-50 dark:bg-zinc-900' : ''
                } ${thread.hasUnread ? 'bg-blue-50/50 dark:bg-blue-950/20' : ''} ${
                  isLatestChecked ? 'bg-blue-100/60 dark:bg-blue-900/30' : ''
                }`}
                data-testid={`email-thread-header-${thread.id}`}
              >
                <div className="flex items-start gap-3">
                  {/* Chevron — only renders for multi-message threads so
                      the solo case visually matches the flat list. */}
                  {!isSolo && (
                    <button
                      type="button"
                      onClick={(e) => {
                        e.stopPropagation();
                        toggleThreadExpansion(thread.id);
                      }}
                      className="mt-1 text-zinc-500 hover:text-zinc-700 dark:hover:text-zinc-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 rounded-sm"
                      aria-label={isExpanded ? `Collapse thread "${thread.subject}"` : `Expand thread "${thread.subject}"`}
                      aria-expanded={isExpanded}
                      data-testid={`email-thread-toggle-${thread.id}`}
                    >
                      {isExpanded ? (
                        <ChevronDown className="size-4" />
                      ) : (
                        <ChevronRight className="size-4" />
                      )}
                    </button>
                  )}

                  {multiSelectEnabled && (
                    <div
                      data-testid={`email-row-select-${latest.id}`}
                      onClick={(e) => {
                        e.stopPropagation();
                        // Bulk selection toggles the latest message in the
                        // thread. A future TMAIL ticket can extend this to
                        // multi-uid selection ("select all in thread") but
                        // single-row toggling keeps the contract simple
                        // for the current bulk-action bar.
                        onToggleSelect?.(latest.id, e.shiftKey);
                      }}
                      className="mt-1 flex items-center"
                    >
                      <Checkbox
                        checked={isLatestChecked}
                        aria-label={
                          isLatestChecked
                            ? `Deselect thread "${thread.subject}"`
                            : `Select thread "${thread.subject}"`
                        }
                        onClick={(e) => e.stopPropagation()}
                        tabIndex={-1}
                      />
                    </div>
                  )}

                  <button
                    type="button"
                    aria-pressed={thread.hasStarred}
                    aria-label={thread.hasStarred ? `Unstar latest message in "${thread.subject}"` : `Star latest message in "${thread.subject}"`}
                    onClick={(e) => {
                      e.stopPropagation();
                      onToggleStar?.(latest.id, thread.hasStarred);
                    }}
                    className="mt-1 cursor-pointer rounded-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
                  >
                    <Star
                      className={`size-4 ${
                        thread.hasStarred
                          ? 'fill-yellow-400 text-yellow-400'
                          : 'text-zinc-400 hover:text-yellow-400'
                      }`}
                    />
                  </button>

                  <div className="flex-1 min-w-0">
                    <div className="flex items-center justify-between mb-1">
                      <span className={`font-medium truncate ${thread.hasUnread ? 'font-semibold' : ''}`}>
                        {participantsLabel}
                        {!isSolo && (
                          <span
                            className="ml-2 inline-flex items-center gap-1 rounded-full bg-zinc-200 dark:bg-zinc-800 px-2 py-0.5 text-xs font-normal text-zinc-700 dark:text-zinc-300"
                            data-testid={`email-thread-count-${thread.id}`}
                            aria-label={`${thread.messages.length} messages in this conversation`}
                          >
                            <MessageSquare className="size-3" aria-hidden="true" />
                            {thread.messages.length}
                          </span>
                        )}
                      </span>
                      <span className="text-xs text-zinc-500 ml-2 whitespace-nowrap">
                        {formatDistanceToNow(thread.latestTimestamp, { addSuffix: true })}
                      </span>
                    </div>

                    <div className={`text-sm truncate mb-1 ${thread.hasUnread ? 'font-medium' : 'text-zinc-600 dark:text-zinc-400'}`}>
                      {thread.subject}
                    </div>

                    <div className="text-sm text-zinc-500 truncate">
                      {latest.preview}
                    </div>
                  </div>
                </div>
              </div>

              {/* Expanded child rows — indented so the user sees the
                  hierarchy. Click any child to open that specific message
                  in the reader pane. */}
              {!isSolo && isExpanded && (
                <div data-testid={`email-thread-children-${thread.id}`}>
                  {thread.messages.map((m) => {
                    const isChildChecked = selectedIds?.has(m.id) ?? false;
                    return (
                      <div
                        key={m.id}
                        onClick={() => onSelectEmail(m.id)}
                        className={`border-b border-zinc-200 dark:border-zinc-800 pl-12 pr-4 py-3 cursor-pointer transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-900 ${
                          selectedEmailId === m.id ? 'bg-zinc-50 dark:bg-zinc-900' : ''
                        } ${!m.read ? 'bg-blue-50/30 dark:bg-blue-950/10' : ''} ${
                          isChildChecked ? 'bg-blue-100/60 dark:bg-blue-900/30' : ''
                        }`}
                        data-testid={`email-thread-child-${m.id}`}
                      >
                        <div className="flex items-start gap-2">
                          <button
                            type="button"
                            aria-pressed={m.starred}
                            aria-label={m.starred ? `Unstar email from ${m.from}` : `Star email from ${m.from}`}
                            onClick={(e) => {
                              e.stopPropagation();
                              onToggleStar?.(m.id, m.starred);
                            }}
                            className="mt-1 cursor-pointer rounded-sm focus:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
                          >
                            <Star
                              className={`size-3.5 ${
                                m.starred
                                  ? 'fill-yellow-400 text-yellow-400'
                                  : 'text-zinc-400 hover:text-yellow-400'
                              }`}
                            />
                          </button>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center justify-between mb-0.5">
                              <span className={`text-sm truncate ${!m.read ? 'font-semibold' : ''}`}>
                                {m.from}
                              </span>
                              <span className="text-xs text-zinc-500 ml-2 whitespace-nowrap">
                                {formatDistanceToNow(m.timestamp, { addSuffix: true })}
                              </span>
                            </div>
                            <div className="text-xs text-zinc-500 truncate">
                              {m.preview}
                            </div>
                            {m.attachments && m.attachments.length > 0 && (
                              <div className="flex items-center gap-1 mt-1 text-xs text-zinc-500">
                                <Paperclip className="size-3" />
                                <span>{m.attachments.length}</span>
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}

        {/* Reuse the same sentinel + loading indicator as the flat view so
            infinite scroll behaves identically when threading is on. */}
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

  // ── Flat view (pre-TMAIL-350 default behaviour) ───────────────────────
  return (
    <div className="overflow-y-auto" data-testid="email-list-flat">
      {emails.map((email) => {
        const isChecked = selectedIds?.has(email.id) ?? false;
        return (
          <div
            key={email.id}
            onClick={() => onSelectEmail(email.id)}
            className={`border-b border-zinc-200 dark:border-zinc-800 p-4 cursor-pointer transition-colors hover:bg-zinc-50 dark:hover:bg-zinc-900 ${
              selectedEmailId === email.id ? 'bg-zinc-50 dark:bg-zinc-900' : ''
            } ${!email.read ? 'bg-blue-50/50 dark:bg-blue-950/20' : ''} ${
              isChecked ? 'bg-blue-100/60 dark:bg-blue-900/30' : ''
            }`}
          >
            <div className="flex items-start gap-3">
              {multiSelectEnabled && (
                // TMAIL-326: checkbox lives in its own wrapper so we can
                // stop the row-click handler from firing the reader. The
                // wrapper handles the click capture (incl. shift-key
                // detection); the inner <Checkbox> ignores its own
                // onCheckedChange so the wrapper is the single source of
                // truth for shift-click range select.
                <div
                  data-testid={`email-row-select-${email.id}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggleSelect?.(email.id, e.shiftKey);
                  }}
                  className="mt-1 flex items-center"
                >
                  <Checkbox
                    checked={isChecked}
                    aria-label={
                      isChecked
                        ? `Deselect email from ${email.from}`
                        : `Select email from ${email.from}`
                    }
                    // The Checkbox primitive forwards onCheckedChange itself,
                    // but we already handle the click on the wrapper —
                    // intercept to avoid double-firing on Radix's click +
                    // synthesized onCheckedChange path.
                    onClick={(e) => e.stopPropagation()}
                    tabIndex={-1}
                  />
                </div>
              )}
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
        );
      })}

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
