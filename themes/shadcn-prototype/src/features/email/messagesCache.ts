// TMAIL-325: helpers + constants for the EmailClient infinite-scroll cache.
//
// The messages query moved from a single MessageListResponse to a TanStack
// InfiniteData wrapper {pages: [MessageListResponse]}, so the optimistic
// mutations (star, archive, delete) need to walk every loaded page rather
// than the single old payload.
//
// Why this lives in its own file (not inline in EmailClient.tsx):
//   * Pure functions — no React, no react-query runtime — so the unit tests
//     in the classic frontend's vitest project can import them via relative
//     path without dragging in the `@/` alias or jsdom React renderer.
//   * Matches the same pattern that replyContext.ts uses for the
//     ComposeModal helpers (TMAIL-319) — co-located, alias-free.
import type { InfiniteData } from '@tanstack/react-query';
import type { MessageEnvelope, MessageListResponse } from '../../types/mail';

/**
 * Backend caps page_size at 200 (handlers/messages.rs::list_messages). 50
 * keeps the first paint quick while leaving plenty of headroom for the
 * intersection-observer sentinel to fetch the next page as the user scrolls.
 */
export const MESSAGES_PAGE_SIZE = 50;

/**
 * Apply `updater` to every loaded page's `messages` array, returning a fresh
 * InfiniteData object (immutable update — leaves the input untouched so the
 * react-query "previous data" rollback path in onError still works).
 *
 * Returns the input unchanged when `data` is undefined so the call sites can
 * stay branch-free: `queryClient.getQueryData` is `T | undefined` by design,
 * and an undefined cache means "nothing to optimistically update".
 */
export function updateInfiniteMessages(
  data: InfiniteData<MessageListResponse> | undefined,
  updater: (msgs: MessageEnvelope[]) => MessageEnvelope[],
): InfiniteData<MessageListResponse> | undefined {
  if (!data) return data;
  return {
    ...data,
    pages: data.pages.map((p) => ({ ...p, messages: updater(p.messages) })),
  };
}

/**
 * Compute the `getNextPageParam` callback for useInfiniteQuery. Returns the
 * next zero-based page number if there are still envelopes the user hasn't
 * fetched, or `undefined` to signal the query has reached the end.
 *
 * Lifted out as a pure helper so the unit tests can pin down the edge cases
 * (page 0 of a small folder, the last partial page, exactly-full pages)
 * without spinning up a QueryClient.
 */
export function nextMessagesPageParam(
  lastPage: MessageListResponse,
): number | undefined {
  const consumed = (lastPage.page + 1) * lastPage.page_size;
  return consumed < lastPage.total ? lastPage.page + 1 : undefined;
}
