// TMAIL-326: unit tests for the alt-UI bulk multi-select helpers.
//
// The helpers live in themes/shadcn-prototype/src/features/email/
// bulkSelection.ts (the modern UI bundle). The shadcn-prototype package
// doesn't ship its own test runner — so we host the tests here in the
// classic frontend's vitest project, following the same pattern as
// themes-replyContext.test.ts and themes-messagesCache.test.ts.
//
// What we're proving:
//   * toggleSelection is immutable and round-trips (toggle twice == noop).
//   * rangeSelect walks the visible uid array between anchor and target
//     regardless of click order, only adds (never removes) uids, and falls
//     back to toggle when the anchor is missing.
//   * selectAll / clearSelection produce fresh Sets.
//   * isAllSelected / isPartiallySelected reflect the right indeterminate
//     state for the master checkbox.
//   * pruneSelection drops uids that left the visible page (folder change,
//     paginated retraction).
import { describe, expect, it } from 'vitest';
import {
  clearSelection,
  isAllSelected,
  isPartiallySelected,
  pruneSelection,
  rangeSelect,
  selectAll,
  toggleSelection,
} from '../../../themes/shadcn-prototype/src/features/email/bulkSelection';

describe('toggleSelection', () => {
  it('adds a uid that is not yet selected', () => {
    const result = toggleSelection(new Set<number>(), 42);
    expect(Array.from(result)).toEqual([42]);
  });

  it('removes a uid that is already selected', () => {
    const result = toggleSelection(new Set<number>([10, 42]), 42);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([10]);
  });

  it('is immutable — returns a fresh Set, leaves the input alone', () => {
    const input = new Set<number>([1, 2]);
    const snapshot = new Set(input);
    const result = toggleSelection(input, 3);
    expect(input).toEqual(snapshot);
    expect(result).not.toBe(input);
  });

  it('round-trips (toggle twice = noop)', () => {
    const input = new Set<number>([1, 2, 3]);
    const once = toggleSelection(input, 2);
    const twice = toggleSelection(once, 2);
    expect(twice).toEqual(input);
  });
});

describe('rangeSelect', () => {
  const orderedUids = [10, 20, 30, 40, 50, 60, 70];

  it('falls back to toggle when the anchor is null', () => {
    const result = rangeSelect(new Set<number>(), orderedUids, null, 40);
    expect(Array.from(result)).toEqual([40]);
  });

  it('falls back to toggle when the anchor equals the target', () => {
    const result = rangeSelect(new Set<number>([40]), orderedUids, 40, 40);
    // Toggle off — 40 was already in the selection.
    expect(result.size).toBe(0);
  });

  it('falls back to toggle when the anchor is no longer visible', () => {
    // 999 isn't in orderedUids (scrolled out or pruned).
    const result = rangeSelect(new Set<number>(), orderedUids, 999, 30);
    expect(Array.from(result)).toEqual([30]);
  });

  it('selects an ascending range (anchor before target)', () => {
    const result = rangeSelect(new Set<number>(), orderedUids, 20, 50);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([20, 30, 40, 50]);
  });

  it('selects a descending range (target before anchor)', () => {
    // Same range; user shift-clicked an earlier row.
    const result = rangeSelect(new Set<number>(), orderedUids, 50, 20);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([20, 30, 40, 50]);
  });

  it('preserves existing selection — range is additive', () => {
    const result = rangeSelect(new Set<number>([10, 70]), orderedUids, 30, 50);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([10, 30, 40, 50, 70]);
  });

  it('never removes a uid that is already selected even if it falls outside the range', () => {
    // 60 is selected but outside [20..40] — stays selected.
    const result = rangeSelect(new Set<number>([60]), orderedUids, 20, 40);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([20, 30, 40, 60]);
  });

  it('is immutable on the input set', () => {
    const input = new Set<number>([10]);
    const snapshot = new Set(input);
    rangeSelect(input, orderedUids, 20, 40);
    expect(input).toEqual(snapshot);
  });
});

describe('selectAll', () => {
  it('returns a Set containing every uid', () => {
    expect(Array.from(selectAll([1, 2, 3])).sort((a, b) => a - b)).toEqual([1, 2, 3]);
  });

  it('returns an empty Set for an empty list', () => {
    expect(selectAll([]).size).toBe(0);
  });
});

describe('clearSelection', () => {
  it('returns an empty Set', () => {
    expect(clearSelection().size).toBe(0);
  });

  it('returns a fresh Set each call', () => {
    expect(clearSelection()).not.toBe(clearSelection());
  });
});

describe('isAllSelected', () => {
  it('is true when every visible uid is selected', () => {
    expect(isAllSelected([1, 2, 3], new Set([1, 2, 3]))).toBe(true);
  });

  it('is true when more uids are selected than visible (subset match)', () => {
    // Real example: user selects 1,2,3 then paginates, list shrinks to 1,2.
    // The remaining visible items are all selected.
    expect(isAllSelected([1, 2], new Set([1, 2, 3]))).toBe(true);
  });

  it('is false when any visible uid is missing from the selection', () => {
    expect(isAllSelected([1, 2, 3], new Set([1, 3]))).toBe(false);
  });

  it('is false when the visible list is empty', () => {
    // Drives the master checkbox: nothing to select → unchecked.
    expect(isAllSelected([], new Set([1, 2]))).toBe(false);
  });
});

describe('isPartiallySelected', () => {
  it('is true when some but not all visible uids are selected', () => {
    expect(isPartiallySelected([1, 2, 3], new Set([1]))).toBe(true);
  });

  it('is false when every visible uid is selected', () => {
    expect(isPartiallySelected([1, 2, 3], new Set([1, 2, 3]))).toBe(false);
  });

  it('is false when no visible uid is selected', () => {
    expect(isPartiallySelected([1, 2, 3], new Set())).toBe(false);
  });

  it('is false when the visible list is empty', () => {
    expect(isPartiallySelected([], new Set([1]))).toBe(false);
  });
});

describe('pruneSelection', () => {
  it('drops uids that are no longer visible', () => {
    // User selected 1,2,3 on the Inbox; folder changes to Sent with uids 2,3,4.
    const result = pruneSelection(new Set([1, 2, 3]), [2, 3, 4]);
    expect(Array.from(result).sort((a, b) => a - b)).toEqual([2, 3]);
  });

  it('returns an empty Set when no visible uid is in the selection', () => {
    const result = pruneSelection(new Set([1, 2]), [10, 20]);
    expect(result.size).toBe(0);
  });

  it('returns an empty Set when the visible list is empty', () => {
    const result = pruneSelection(new Set([1, 2]), []);
    expect(result.size).toBe(0);
  });

  it('is immutable on the input set', () => {
    const input = new Set([1, 2, 3]);
    const snapshot = new Set(input);
    pruneSelection(input, [2, 3]);
    expect(input).toEqual(snapshot);
  });
});
