// TMAIL-325: unit tests for the alt-UI infinite-scroll cache helpers.
//
// The helpers live in themes/shadcn-prototype/src/features/email/
// messagesCache.ts (the modern UI bundle). The shadcn-prototype package
// doesn't ship its own test runner — so we host the tests here in the
// classic frontend's vitest project (already configured for the rest of
// the SPA), following the same pattern as themes-replyContext.test.ts.
//
// What we're proving:
//   * updateInfiniteMessages walks every loaded page (not just the first
//     one) — the bug we'd regress if the optimistic mutations went back
//     to the single-payload cache shape.
//   * It's immutable: the caller's snapshot used by the onError rollback
//     path is left untouched.
//   * nextMessagesPageParam returns the right next page on partial /
//     exact-boundary / over-consumed inputs so the IntersectionObserver
//     sentinel stops fetching when the inbox is fully loaded.
import type { InfiniteData } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import {
  MESSAGES_PAGE_SIZE,
  nextMessagesPageParam,
  updateInfiniteMessages,
} from '../../../themes/shadcn-prototype/src/features/email/messagesCache';
import type { MessageEnvelope, MessageListResponse } from '../../../themes/shadcn-prototype/src/types/mail';

function makeEnvelope(uid: number, flags: string[] = []): MessageEnvelope {
  return {
    uid,
    subject: `Subject ${uid}`,
    from: `sender${uid}@example.com`,
    date: '2026-05-30T12:00:00Z',
    flags,
    size: 1024,
  };
}

function makePage(
  page: number,
  uids: number[],
  total: number,
  pageSize = 50,
): MessageListResponse {
  return {
    messages: uids.map((u) => makeEnvelope(u)),
    total,
    page,
    page_size: pageSize,
  };
}

function makeInfinite(
  pages: MessageListResponse[],
): InfiniteData<MessageListResponse> {
  return {
    pages,
    pageParams: pages.map((p) => p.page),
  };
}

describe('MESSAGES_PAGE_SIZE', () => {
  it('matches the backend default (50)', () => {
    // The Rust handler defaults page_size to 50 and caps it at 200 —
    // keep this constant in lockstep so the sentinel doesn't end up
    // requesting tiny pages the backend has to up-paginate around.
    expect(MESSAGES_PAGE_SIZE).toBe(50);
  });
});

describe('updateInfiniteMessages', () => {
  it('returns the input unchanged when data is undefined', () => {
    expect(updateInfiniteMessages(undefined, (m) => m)).toBeUndefined();
  });

  it('applies the updater to every page', () => {
    const input = makeInfinite([
      makePage(0, [1, 2, 3], 200),
      makePage(1, [4, 5, 6], 200),
      makePage(2, [7, 8, 9], 200),
    ]);
    // Star (+\Flagged) on every uid across every page.
    const result = updateInfiniteMessages(input, (msgs) =>
      msgs.map((m) => ({ ...m, flags: [...m.flags, '\\Flagged'] })),
    );
    expect(result).toBeDefined();
    expect(result!.pages).toHaveLength(3);
    for (const page of result!.pages) {
      for (const m of page.messages) {
        expect(m.flags).toContain('\\Flagged');
      }
    }
  });

  it('removes a uid from a later page (filter updater)', () => {
    // The deletion lives on page 2 — proves we don't only walk page 0.
    const input = makeInfinite([
      makePage(0, [1, 2, 3], 200),
      makePage(1, [4, 5, 6], 200),
      makePage(2, [7, 8, 9], 200),
    ]);
    const result = updateInfiniteMessages(input, (msgs) =>
      msgs.filter((m) => m.uid !== 8),
    );
    const allUids = result!.pages.flatMap((p) => p.messages.map((m) => m.uid));
    expect(allUids).toEqual([1, 2, 3, 4, 5, 6, 7, 9]);
  });

  it('does not mutate the input snapshot (rollback safety)', () => {
    // The onError rollback in EmailClient uses the snapshot captured in
    // onMutate — if updateInfiniteMessages mutates in place, that
    // rollback re-applies the failed change instead of reverting.
    const input = makeInfinite([
      makePage(0, [1, 2, 3], 100),
      makePage(1, [4, 5, 6], 100),
    ]);
    const snapshot = JSON.parse(JSON.stringify(input));
    updateInfiniteMessages(input, (msgs) =>
      msgs.filter((m) => m.uid !== 5),
    );
    expect(input).toEqual(snapshot);
  });

  it('preserves total/page/page_size on each page', () => {
    // Sanity: the optimistic update keeps the pagination metadata so
    // nextMessagesPageParam keeps returning the right next page even
    // after a row is dropped optimistically.
    const input = makeInfinite([
      makePage(0, [1, 2, 3], 123, 50),
      makePage(1, [4, 5, 6], 123, 50),
    ]);
    const result = updateInfiniteMessages(input, (msgs) =>
      msgs.filter((m) => m.uid !== 2),
    );
    expect(result!.pages[0].total).toBe(123);
    expect(result!.pages[0].page).toBe(0);
    expect(result!.pages[0].page_size).toBe(50);
    expect(result!.pages[1].total).toBe(123);
    expect(result!.pages[1].page).toBe(1);
  });
});

describe('nextMessagesPageParam', () => {
  it('returns the next page index when more envelopes remain', () => {
    // Page 0 of 50, total 200 → 50 consumed, 150 to go → fetch page 1.
    expect(nextMessagesPageParam(makePage(0, [], 200))).toBe(1);
  });

  it('returns undefined when total exactly matches consumed', () => {
    // Page 3, page_size 50, 4 pages × 50 = 200 consumed = total → stop.
    expect(nextMessagesPageParam(makePage(3, [], 200))).toBeUndefined();
  });

  it('returns undefined for a partial last page', () => {
    // Total 75 with page_size 50: page 0 has 50, page 1 has 25 — after
    // page 1 the consumed count (100) exceeds total (75) and the
    // sentinel stops trying to load more.
    expect(nextMessagesPageParam(makePage(1, [], 75))).toBeUndefined();
  });

  it('handles the empty-folder edge case', () => {
    // Total 0 — page 0 of 50 is already over budget → no next page.
    expect(nextMessagesPageParam(makePage(0, [], 0))).toBeUndefined();
  });

  it('respects the lastPage page_size, not a hardcoded constant', () => {
    // Backend may up-paginate to 200 on a request — the caller cares
    // about what was actually returned, not the request hint. 250
    // remaining with a 100-row page means another page is needed.
    expect(nextMessagesPageParam(makePage(0, [], 250, 100))).toBe(1);
  });
});
