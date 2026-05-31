// Added (TMAIL-400): registry contract tests.
//
// The registry is the single source of truth for /admin/* — both the
// AdminShell left rail AND App.tsx's admin Route block read from it. A
// silent drift here (duplicate slug, missing group, mistyped id, drop
// of a critical category) would break navigation everywhere at once.
// These tests fail loudly on every drift category so the refactor stays
// safe.
import { describe, it, expect } from 'vitest';
import {
  ADMIN_GROUP_LABELS,
  ADMIN_GROUP_ORDER,
  ADMIN_NAV,
  DEFAULT_ADMIN_ID,
  groupedAdminNav,
  type AdminGroup,
} from './admin-nav-registry';

// The 18 admin-only managers TMAIL-400 calls out by name, expressed as
// registry slugs. If any of these disappear, a real operator capability
// has gone missing and the gate fails here.
const EXPECTED_NEW_ADMIN_SLUGS = [
  'dlp',
  'ediscovery',
  'saml',
  'oidc',
  'ldap',
  'dane',
  'retention',
  'branding',
  'hostnames',
  'bulk-import',
  'activesync',
  'plugins',
  'webhooks',
  'chat-integrations',
  'shared-mailboxes',
  'deliverability',
  'archive',
  'billing',
] as const;

// The 8 admin managers that existed pre-400 — keep them in the registry
// too so the existing admin-shell-flow.spec.ts keeps passing.
const EXPECTED_LEGACY_ADMIN_SLUGS = [
  'feature-flags',
  'quote-requests',
  'audit-log',
  'cache',
  'domains',
  'payment-providers',
  'users',
  'warmup',
] as const;

describe('admin-nav-registry (TMAIL-400)', () => {
  it('exports all 8 pre-TMAIL-400 admin slugs', () => {
    const slugs = ADMIN_NAV.map((item) => item.id);
    for (const expected of EXPECTED_LEGACY_ADMIN_SLUGS) {
      expect(slugs).toContain(expected);
    }
  });

  it('exports all 18 admin slugs called out in the ticket', () => {
    const slugs = ADMIN_NAV.map((item) => item.id);
    for (const expected of EXPECTED_NEW_ADMIN_SLUGS) {
      expect(slugs).toContain(expected);
    }
  });

  it('has a unique id per entry', () => {
    const ids = ADMIN_NAV.map((item) => item.id);
    const unique = new Set(ids);
    expect(unique.size).toBe(ids.length);
  });

  it('has a unique label per entry', () => {
    const labels = ADMIN_NAV.map((item) => item.label);
    const unique = new Set(labels);
    expect(unique.size).toBe(labels.length);
  });

  it('every entry has a non-empty label, icon, and lazy component', () => {
    for (const item of ADMIN_NAV) {
      expect(item.label, `${item.id} label`).toBeTruthy();
      expect(item.icon, `${item.id} icon`).toBeTruthy();
      expect(item.component, `${item.id} component`).toBeTruthy();
    }
  });

  it('every entry belongs to a declared group', () => {
    const allowed = new Set<AdminGroup>(ADMIN_GROUP_ORDER);
    for (const item of ADMIN_NAV) {
      expect(allowed.has(item.group), `${item.id} group=${item.group}`).toBe(true);
    }
  });

  it('every declared group has a human-readable label', () => {
    for (const group of ADMIN_GROUP_ORDER) {
      expect(ADMIN_GROUP_LABELS[group], `group label for ${group}`).toBeTruthy();
    }
  });

  it('DEFAULT_ADMIN_ID points at a real entry', () => {
    const ids = ADMIN_NAV.map((item) => item.id);
    expect(ids).toContain(DEFAULT_ADMIN_ID);
  });

  it('groupedAdminNav returns groups in ADMIN_GROUP_ORDER and drops empty ones', () => {
    const grouped = groupedAdminNav();
    // Order: every present group must appear in the same relative order
    // as ADMIN_GROUP_ORDER.
    const groupOrder = grouped.map((g) => g.group);
    const indexInDeclaredOrder = (g: AdminGroup) => ADMIN_GROUP_ORDER.indexOf(g);
    const sorted = [...groupOrder].sort(
      (a, b) => indexInDeclaredOrder(a) - indexInDeclaredOrder(b),
    );
    expect(groupOrder).toEqual(sorted);
    // No empty buckets.
    for (const { items } of grouped) {
      expect(items.length).toBeGreaterThan(0);
    }
    // Every registry entry shows up exactly once.
    const grouped_ids = grouped.flatMap((g) => g.items.map((i) => i.id));
    const registry_ids = ADMIN_NAV.map((i) => i.id);
    expect(new Set(grouped_ids)).toEqual(new Set(registry_ids));
    expect(grouped_ids.length).toBe(registry_ids.length);
  });

  it('registry size matches 8 legacy + 18 new = 26 entries', () => {
    expect(ADMIN_NAV.length).toBe(
      EXPECTED_LEGACY_ADMIN_SLUGS.length + EXPECTED_NEW_ADMIN_SLUGS.length,
    );
  });
});
