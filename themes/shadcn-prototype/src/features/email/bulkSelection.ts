// TMAIL-326: pure helpers for the alt-UI EmailList multi-select.
//
// EmailList exposes a per-row checkbox and EmailClient layers a bulk-action
// bar (mark read/unread, star, archive, delete, move) on top of the existing
// single-row select-to-read behaviour. The selection itself is a Set<number>
// of IMAP uids that lives in EmailClient; the helpers here are pure
// (no React, no react-query, no DOM) so the unit tests can pin down the
// edge cases — empty list, missing anchor, shift-click before any single
// click, anchor that's no longer visible after pagination, etc. — without
// standing up the renderer.
//
// Why pure functions in their own module:
//   * Mirrors the pattern messagesCache.ts (TMAIL-325) and replyContext.ts
//     (TMAIL-319) established — alias-free relative imports so the classic
//     frontend's vitest project can host the tests via
//     ../../../themes/shadcn-prototype/...
//   * Keeps EmailList focused on rendering rows and forwarding intent, and
//     EmailClient focused on wiring mutations + cache invalidation.
//   * Range-select reads as a single declarative function rather than 20
//     lines of imperative index math inlined in the click handler.

/**
 * Toggle a single uid in/out of the selection. Always returns a NEW Set so
 * React reference-equality re-rendering fires — callers should not mutate
 * the input. Matches the `currentlyStarred ? remove : add` pattern used by
 * the star mutation.
 */
export function toggleSelection(
  selection: ReadonlySet<number>,
  uid: number,
): Set<number> {
  const next = new Set(selection);
  if (next.has(uid)) {
    next.delete(uid);
  } else {
    next.add(uid);
  }
  return next;
}

/**
 * Shift-click range select. The range from `anchorUid` to `targetUid`
 * (inclusive, regardless of which appears first in the list) is ADDED to
 * the existing selection — matches Gmail / Outlook behaviour where shift +
 * click never deselects.
 *
 * Falls back to {@link toggleSelection} on `targetUid` when:
 *   * `anchorUid` is null (the user has not single-clicked any row yet), or
 *   * `anchorUid === targetUid` (the range collapses to one row), or
 *   * either uid is not present in `orderedUids` (e.g. the anchor row scrolled
 *     out of the loaded pages — we don't try to fetch missing pages just to
 *     materialise the range; the user can re-anchor by single-clicking).
 *
 * `orderedUids` MUST be the visible list in display order so the range walks
 * the same rows the user sees on screen.
 */
export function rangeSelect(
  selection: ReadonlySet<number>,
  orderedUids: ReadonlyArray<number>,
  anchorUid: number | null,
  targetUid: number,
): Set<number> {
  if (anchorUid == null || anchorUid === targetUid) {
    return toggleSelection(selection, targetUid);
  }
  const anchorIdx = orderedUids.indexOf(anchorUid);
  const targetIdx = orderedUids.indexOf(targetUid);
  if (anchorIdx === -1 || targetIdx === -1) {
    return toggleSelection(selection, targetUid);
  }
  const lo = Math.min(anchorIdx, targetIdx);
  const hi = Math.max(anchorIdx, targetIdx);
  const next = new Set(selection);
  for (let i = lo; i <= hi; i++) {
    next.add(orderedUids[i]);
  }
  return next;
}

/**
 * Replace the selection with every currently visible uid. Drives the
 * "select all" affordance in the bulk-action bar.
 */
export function selectAll(uids: ReadonlyArray<number>): Set<number> {
  return new Set(uids);
}

/**
 * Empty selection. Tiny helper to keep the call sites self-documenting:
 * `setSelectedUids(clearSelection())` reads better than `new Set()`.
 */
export function clearSelection(): Set<number> {
  return new Set();
}

/**
 * True iff every visible uid is in the selection AND the list has at least
 * one item. Drives the top-of-list checkbox `checked` state.
 */
export function isAllSelected(
  uids: ReadonlyArray<number>,
  selection: ReadonlySet<number>,
): boolean {
  if (uids.length === 0) return false;
  for (const u of uids) {
    if (!selection.has(u)) return false;
  }
  return true;
}

/**
 * True iff *some but not all* of the visible uids are selected. Drives the
 * indeterminate ("dash") state of the top-of-list checkbox.
 */
export function isPartiallySelected(
  uids: ReadonlyArray<number>,
  selection: ReadonlySet<number>,
): boolean {
  if (uids.length === 0) return false;
  let count = 0;
  for (const u of uids) {
    if (selection.has(u)) count++;
  }
  return count > 0 && count < uids.length;
}

/**
 * Drop any uid from the selection that is no longer visible. Called by
 * EmailClient when the folder changes — the user's selection from the
 * previous folder shouldn't leak into bulk actions on the new folder. Also
 * useful after a paginated fetch settles if some uids vanished server-side.
 */
export function pruneSelection(
  selection: ReadonlySet<number>,
  visibleUids: ReadonlyArray<number>,
): Set<number> {
  const visible = new Set(visibleUids);
  const next = new Set<number>();
  for (const u of selection) {
    if (visible.has(u)) next.add(u);
  }
  return next;
}
