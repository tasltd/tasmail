// TMAIL-351: pure-logic unit tests for the recurrence registry. Lives in
// the shadcn-prototype workspace, which has no vitest/jest runner, so we
// use the Node 22 native test runner with TypeScript stripping (the
// preset → RRULE mapping is small enough that we don't need a full test
// framework for it).
//
// Run with:  node --experimental-strip-types --test src/features/calendar/recurrence.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { RECURRENCE_PRESETS, presetForRrule, resolveRrule } from './recurrence.ts';

test('presetForRrule maps null/undefined to "none"', () => {
  assert.equal(presetForRrule(null), 'none');
  assert.equal(presetForRrule(undefined), 'none');
});

test('presetForRrule maps known FREQ=WEEKLY to "weekly"', () => {
  assert.equal(presetForRrule('FREQ=WEEKLY'), 'weekly');
});

test('presetForRrule maps biweekly RRULE to "biweekly"', () => {
  assert.equal(presetForRrule('FREQ=WEEKLY;INTERVAL=2'), 'biweekly');
});

test('presetForRrule falls back to "custom" for unknown rules', () => {
  assert.equal(presetForRrule('FREQ=MONTHLY;BYDAY=2MO'), 'custom');
});

test('resolveRrule returns null for "none"', () => {
  assert.equal(resolveRrule('none', 'anything'), null);
});

test('resolveRrule honours the preset RRULE body', () => {
  assert.equal(resolveRrule('weekly', ''), 'FREQ=WEEKLY');
  assert.equal(resolveRrule('biweekly', ''), 'FREQ=WEEKLY;INTERVAL=2');
  assert.equal(resolveRrule('monthly', ''), 'FREQ=MONTHLY');
});

test('resolveRrule with "custom" uses the typed value', () => {
  assert.equal(resolveRrule('custom', 'FREQ=YEARLY;BYMONTH=12'), 'FREQ=YEARLY;BYMONTH=12');
});

test('resolveRrule with "custom" + empty/whitespace returns null', () => {
  assert.equal(resolveRrule('custom', ''), null);
  assert.equal(resolveRrule('custom', '   '), null);
});

test('RECURRENCE_PRESETS includes the reserved "none" and "custom" entries', () => {
  const values = RECURRENCE_PRESETS.map((p) => p.value);
  assert.ok(values.includes('none'));
  assert.ok(values.includes('custom'));
});

test('RECURRENCE_PRESETS rrule field shape is sane', () => {
  for (const p of RECURRENCE_PRESETS) {
    if (p.value === 'none') {
      assert.equal(p.rrule, null);
    } else if (p.value === 'custom') {
      assert.equal(p.rrule, undefined);
    } else {
      assert.equal(typeof p.rrule, 'string');
      assert.ok(String(p.rrule).startsWith('FREQ='));
    }
  }
});
