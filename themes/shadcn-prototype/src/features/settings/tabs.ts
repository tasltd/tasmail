// TMAIL-323: registry-driven tab definitions for the alt-UI /settings shell.
//
// Each entry is the source of truth for: route slug (path under /settings),
// sidebar label, icon, and a one-line description rendered in the placeholder
// pane. Sub-pane implementations land in P1 tasks — until then every tab
// renders the same placeholder component (`SettingsTabPlaceholder`) which
// reads its copy from this registry. Adding a future P1 sub-pane is a
// one-line edit to this file plus a route entry.
import {
  User,
  AtSign,
  PenSquare,
  Plane,
  Filter,
  ShieldCheck,
  Palette,
  Server,
  type LucideIcon,
} from 'lucide-react';

export interface SettingsTab {
  /** URL slug under /settings (e.g. "profile" → /settings/profile) */
  slug: string;
  /** Sidebar label */
  label: string;
  /** lucide icon component */
  icon: LucideIcon;
  /** One-line description rendered in the placeholder pane */
  description: string;
  /** data-testid suffix — used by Playwright + future component tests */
  testId: string;
}

export const SETTINGS_TABS: SettingsTab[] = [
  {
    slug: 'profile',
    label: 'Profile',
    icon: User,
    description:
      'Display name, contact email, time zone, and locale for your TASMail account.',
    testId: 'settings-tab-profile',
  },
  {
    slug: 'identities',
    label: 'Identities',
    icon: AtSign,
    description:
      'Send-as addresses and default reply-from identity for outgoing mail.',
    testId: 'settings-tab-identities',
  },
  {
    slug: 'signatures',
    label: 'Signatures',
    icon: PenSquare,
    description:
      'HTML and plain-text signatures attached to new mail and replies.',
    testId: 'settings-tab-signatures',
  },
  {
    slug: 'vacation',
    label: 'Vacation',
    icon: Plane,
    description:
      'Auto-reply / out-of-office responder with active window and message text.',
    testId: 'settings-tab-vacation',
  },
  {
    slug: 'filters',
    label: 'Filters',
    icon: Filter,
    description:
      'Sieve mail filter rules: match conditions, then move / flag / forward.',
    testId: 'settings-tab-filters',
  },
  {
    slug: 'mfa',
    label: 'MFA',
    icon: ShieldCheck,
    description:
      'Two-factor authentication — TOTP authenticator, SMS OTP, and FIDO2 keys.',
    testId: 'settings-tab-mfa',
  },
  {
    slug: 'theme',
    label: 'Theme',
    icon: Palette,
    description:
      'Light / dark / system theme, accent color, density, and reading-pane layout.',
    testId: 'settings-tab-theme',
  },
  {
    slug: 'imap-smtp',
    label: 'IMAP / SMTP',
    icon: Server,
    description:
      'BYOK mail-server credentials — IMAP fetch and SMTP submission endpoints.',
    testId: 'settings-tab-imap-smtp',
  },
];

export const DEFAULT_SETTINGS_TAB = SETTINGS_TABS[0]; // Profile

export function findTabBySlug(slug: string | undefined): SettingsTab | undefined {
  if (!slug) return undefined;
  return SETTINGS_TABS.find((t) => t.slug === slug);
}
