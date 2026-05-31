// TMAIL-352: pure-logic unit tests for the audit-log helpers. Uses the
// node 22 native test runner (the shadcn-prototype workspace has no
// vitest/jest configured — same pattern as features/calendar/recurrence.test.mjs).
//
// Run with:
//   node --experimental-strip-types --test src/features/admin/audit-log-helpers.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  localToIso,
  parseAdminTab,
  buildAuditLogQueryString,
  ADMIN_TAB_AUDIT,
  ADMIN_TAB_OVERVIEW,
} from './audit-log-helpers.ts';

// --- localToIso -------------------------------------------------------------

test('localToIso returns undefined for empty / null / undefined input', () => {
  assert.equal(localToIso(''), undefined);
  assert.equal(localToIso(undefined), undefined);
  assert.equal(localToIso(null), undefined);
});

test('localToIso returns undefined for unparseable junk', () => {
  assert.equal(localToIso('not-a-date'), undefined);
});

test('localToIso returns an ISO UTC string for a valid datetime-local value', () => {
  const out = localToIso('2026-05-31T09:30');
  assert.ok(typeof out === 'string', `expected string, got ${typeof out}`);
  // Must end in Z because we coerce to UTC.
  assert.ok(out.endsWith('Z'), `expected trailing Z, got ${out}`);
  // chrono parses RFC3339 just fine, so we only sanity-check the shape.
  assert.match(out, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/);
});

// --- parseAdminTab ----------------------------------------------------------

test('parseAdminTab defaults to overview for null/empty/unknown', () => {
  assert.equal(parseAdminTab(null), ADMIN_TAB_OVERVIEW);
  assert.equal(parseAdminTab(''), ADMIN_TAB_OVERVIEW);
  assert.equal(parseAdminTab('bogus'), ADMIN_TAB_OVERVIEW);
  assert.equal(parseAdminTab(undefined), ADMIN_TAB_OVERVIEW);
});

test('parseAdminTab recognises the audit-log tab id exactly', () => {
  assert.equal(parseAdminTab('audit-log'), ADMIN_TAB_AUDIT);
});

test('parseAdminTab is case-sensitive (mirrors URL param convention)', () => {
  assert.equal(parseAdminTab('Audit-Log'), ADMIN_TAB_OVERVIEW);
  assert.equal(parseAdminTab('AUDIT-LOG'), ADMIN_TAB_OVERVIEW);
});

// --- buildAuditLogQueryString ----------------------------------------------

test('buildAuditLogQueryString returns empty string when nothing is set', () => {
  assert.equal(buildAuditLogQueryString({}), '');
});

test('buildAuditLogQueryString includes mailbox_id when set', () => {
  const qs = buildAuditLogQueryString({ mailbox_id: 'abc-123' });
  assert.equal(qs, 'mailbox_id=abc-123');
});

test('buildAuditLogQueryString URL-encodes the action prefix dot', () => {
  const qs = buildAuditLogQueryString({ action: 'auth.' });
  // dots and percent signs in URLs survive encoding — assert the literal text.
  assert.equal(qs, 'action=auth.');
});

test('buildAuditLogQueryString carries from/to ISO strings through unchanged', () => {
  const qs = buildAuditLogQueryString({
    from: '2026-05-01T00:00:00.000Z',
    to: '2026-05-31T23:59:59.999Z',
  });
  // URLSearchParams encodes the colon — `%3A` — that's fine, chrono accepts it.
  assert.ok(qs.includes('from='), `expected from param in ${qs}`);
  assert.ok(qs.includes('to='), `expected to param in ${qs}`);
  // Round-trip via URLSearchParams to verify the values survive decoding.
  const params = new URLSearchParams(qs);
  assert.equal(params.get('from'), '2026-05-01T00:00:00.000Z');
  assert.equal(params.get('to'), '2026-05-31T23:59:59.999Z');
});

test('buildAuditLogQueryString writes limit/offset as strings', () => {
  const qs = buildAuditLogQueryString({ limit: 50, offset: 100 });
  const params = new URLSearchParams(qs);
  assert.equal(params.get('limit'), '50');
  assert.equal(params.get('offset'), '100');
});

test('buildAuditLogQueryString respects offset=0 (must NOT be omitted)', () => {
  // Regression guard: an `if (params.offset)` check would skip 0.
  const qs = buildAuditLogQueryString({ offset: 0 });
  const params = new URLSearchParams(qs);
  assert.equal(params.get('offset'), '0');
});

test('buildAuditLogQueryString combines every filter at once', () => {
  const qs = buildAuditLogQueryString({
    mailbox_id: 'm1',
    action: 'auth.login',
    from: '2026-01-01T00:00:00.000Z',
    to: '2026-12-31T00:00:00.000Z',
    limit: 25,
    offset: 50,
  });
  const params = new URLSearchParams(qs);
  assert.equal(params.get('mailbox_id'), 'm1');
  assert.equal(params.get('action'), 'auth.login');
  assert.equal(params.get('limit'), '25');
  assert.equal(params.get('offset'), '50');
});
