// Added (TMAIL-400): data-driven registry that powers AdminShell's left rail
// AND the /admin/<id> routes in App.tsx. Per the workspace "Modularize"
// rule, adding a new admin category = one entry here, no AdminShell or
// App.tsx edit required.
//
// Why this lives next to AdminShell (not under settings/) even though
// half the components live in components/settings/:
//   The settings/ folder is a historical bag of manager components some
//   of which were always admin-scoped (Branding, DLP, SAML, ...). Moving
//   the files now would balloon this ticket's blast radius. The registry
//   is the seam that asserts "these are admin-only" without a relocation.
//
// Why one flat list grouped by `group` (and not the SettingsHub
// category-with-nested-sections shape):
//   AdminShell pages are deep, narrow tools (one screen per concern).
//   Two-level nesting would force the operator into an extra click for
//   no gain. We keep visual grouping via group headers in the left rail
//   so the surface is still scannable as it grows past 30 entries.
//
// Every component is `React.lazy(...)` so the admin chunk only loads
// when the operator actually opens /admin — non-admin users never even
// fetch the bundle.
import { lazy } from 'react';
import type { ComponentType, LazyExoticComponent } from 'react';
import {
  Activity,
  Archive,
  AtSign,
  Banknote,
  BarChart3,
  CreditCard,
  Database,
  FileSearch,
  Globe,
  Hourglass,
  Inbox,
  KeyRound,
  Lock,
  MessageSquare,
  Palette,
  Plug,
  ScrollText,
  Server,
  ShieldAlert,
  ShieldCheck,
  Smartphone,
  ToggleRight,
  Upload,
  Users,
  UsersRound,
  Webhook,
  type LucideIcon,
} from 'lucide-react';

export type AdminGroup =
  | 'system'
  | 'tenant'
  | 'identity'
  | 'compliance'
  | 'mail'
  | 'integrations'
  | 'billing';

export interface AdminNavItem {
  // URL slug — rendered at /admin/<id>. Must be unique across ADMIN_NAV.
  id: string;
  // Sidebar label.
  label: string;
  icon: LucideIcon;
  // The right-pane component, lazy-loaded.
  component: LazyExoticComponent<ComponentType>;
  group: AdminGroup;
}

// Render order for group headers in the left rail.
export const ADMIN_GROUP_ORDER: AdminGroup[] = [
  'system',
  'tenant',
  'identity',
  'compliance',
  'mail',
  'integrations',
  'billing',
];

export const ADMIN_GROUP_LABELS: Record<AdminGroup, string> = {
  system: 'System',
  tenant: 'Tenant',
  identity: 'Identity & Access',
  compliance: 'Compliance',
  mail: 'Mail',
  integrations: 'Integrations',
  billing: 'Billing',
};

// PURPOSE: Single source of truth for every admin manager wired into
// /admin/*. Order inside each group is render order.
export const ADMIN_NAV: AdminNavItem[] = [
  // -------- System --------
  {
    id: 'feature-flags',
    label: 'Feature flags',
    icon: ToggleRight,
    group: 'system',
    component: lazy(() =>
      import('./FeatureFlagsManager').then((m) => ({ default: m.FeatureFlagsManager })),
    ),
  },
  {
    id: 'cache',
    label: 'Cache',
    icon: Database,
    group: 'system',
    component: lazy(() =>
      import('./CacheManager').then((m) => ({ default: m.CacheManager })),
    ),
  },
  {
    id: 'audit-log',
    label: 'Audit log',
    icon: ScrollText,
    group: 'system',
    component: lazy(() =>
      import('./AuditLogManager').then((m) => ({ default: m.AuditLogManager })),
    ),
  },
  {
    id: 'warmup',
    label: 'IP warm-up',
    icon: Activity,
    group: 'system',
    component: lazy(() =>
      import('./WarmupManager').then((m) => ({ default: m.WarmupManager })),
    ),
  },

  // -------- Tenant --------
  {
    id: 'domains',
    label: 'Domains',
    icon: Globe,
    group: 'tenant',
    component: lazy(() =>
      import('./DomainsManager').then((m) => ({ default: m.DomainsManager })),
    ),
  },
  {
    id: 'users',
    label: 'Users',
    icon: Users,
    group: 'tenant',
    component: lazy(() =>
      import('./UsersManager').then((m) => ({ default: m.UsersManager })),
    ),
  },
  {
    id: 'branding',
    label: 'Branding',
    icon: Palette,
    group: 'tenant',
    component: lazy(() =>
      import('../settings/BrandingManager').then((m) => ({ default: m.BrandingManager })),
    ),
  },
  {
    id: 'hostnames',
    label: 'Hostnames',
    icon: AtSign,
    group: 'tenant',
    component: lazy(() =>
      import('../settings/HostnameManager').then((m) => ({ default: m.HostnameManager })),
    ),
  },
  {
    id: 'bulk-import',
    label: 'Bulk import',
    icon: Upload,
    group: 'tenant',
    component: lazy(() =>
      import('../settings/BulkImportManager').then((m) => ({ default: m.BulkImportManager })),
    ),
  },

  // -------- Identity & Access --------
  {
    id: 'saml',
    label: 'SAML',
    icon: KeyRound,
    group: 'identity',
    component: lazy(() =>
      import('../settings/SamlManager').then((m) => ({ default: m.SamlManager })),
    ),
  },
  {
    id: 'oidc',
    label: 'OIDC',
    icon: Lock,
    group: 'identity',
    component: lazy(() =>
      import('../settings/OidcManager').then((m) => ({ default: m.OidcManager })),
    ),
  },
  {
    id: 'ldap',
    label: 'LDAP',
    icon: Server,
    group: 'identity',
    component: lazy(() =>
      import('../settings/LdapManager').then((m) => ({ default: m.LdapManager })),
    ),
  },

  // -------- Compliance --------
  {
    id: 'dlp',
    label: 'DLP',
    icon: ShieldAlert,
    group: 'compliance',
    component: lazy(() =>
      import('../settings/DlpManager').then((m) => ({ default: m.DlpManager })),
    ),
  },
  {
    id: 'ediscovery',
    label: 'eDiscovery',
    icon: FileSearch,
    group: 'compliance',
    component: lazy(() =>
      import('../settings/EdiscoveryManager').then((m) => ({ default: m.EdiscoveryManager })),
    ),
  },
  {
    id: 'dane',
    label: 'DANE',
    icon: ShieldCheck,
    group: 'compliance',
    component: lazy(() =>
      import('../settings/DaneManager').then((m) => ({ default: m.DaneManager })),
    ),
  },
  {
    id: 'retention',
    label: 'Retention',
    icon: Hourglass,
    group: 'compliance',
    component: lazy(() =>
      import('../settings/RetentionManager').then((m) => ({ default: m.RetentionManager })),
    ),
  },
  {
    id: 'archive',
    label: 'Archive',
    icon: Archive,
    group: 'compliance',
    component: lazy(() =>
      import('../settings/ArchiveManager').then((m) => ({ default: m.ArchiveManager })),
    ),
  },

  // -------- Mail --------
  {
    id: 'deliverability',
    label: 'Deliverability',
    icon: BarChart3,
    group: 'mail',
    component: lazy(() =>
      import('../settings/DeliverabilityReport').then((m) => ({
        default: m.DeliverabilityReport,
      })),
    ),
  },
  {
    id: 'activesync',
    label: 'ActiveSync',
    icon: Smartphone,
    group: 'mail',
    component: lazy(() =>
      import('../settings/ActiveSyncManager').then((m) => ({ default: m.ActiveSyncManager })),
    ),
  },
  {
    id: 'shared-mailboxes',
    label: 'Shared mailboxes',
    icon: UsersRound,
    group: 'mail',
    component: lazy(() =>
      import('../settings/SharedMailboxManager').then((m) => ({
        default: m.SharedMailboxManager,
      })),
    ),
  },

  // -------- Integrations --------
  {
    id: 'plugins',
    label: 'Plugins',
    icon: Plug,
    group: 'integrations',
    component: lazy(() =>
      import('../settings/PluginManager').then((m) => ({ default: m.PluginManager })),
    ),
  },
  {
    id: 'webhooks',
    label: 'Webhooks',
    icon: Webhook,
    group: 'integrations',
    component: lazy(() =>
      import('../settings/WebhookManager').then((m) => ({ default: m.WebhookManager })),
    ),
  },
  {
    id: 'chat-integrations',
    label: 'Chat integrations',
    icon: MessageSquare,
    group: 'integrations',
    component: lazy(() =>
      import('../settings/ChatIntegrationManager').then((m) => ({
        default: m.ChatIntegrationManager,
      })),
    ),
  },

  // -------- Billing --------
  {
    id: 'billing',
    label: 'Billing',
    icon: Banknote,
    group: 'billing',
    component: lazy(() =>
      import('../settings/BillingManager').then((m) => ({ default: m.BillingManager })),
    ),
  },
  {
    id: 'payment-providers',
    label: 'Payment providers',
    icon: CreditCard,
    group: 'billing',
    component: lazy(() =>
      import('./PaymentProvidersManager').then((m) => ({ default: m.PaymentProvidersManager })),
    ),
  },
  {
    id: 'quote-requests',
    label: 'Quote requests',
    icon: Inbox,
    group: 'billing',
    component: lazy(() =>
      import('./QuoteRequestsManager').then((m) => ({ default: m.QuoteRequestsManager })),
    ),
  },
];

// Default landing target when the operator hits /admin without a sub-path.
// Feature flags was the historical default (TMAIL-197) and stays as the
// "you're an operator, here's the most-used screen first" surface.
export const DEFAULT_ADMIN_ID = 'feature-flags';

// PURPOSE: Iterate ADMIN_NAV bucketed by group, in render order, dropping
// empty groups so the renderer never emits an orphan header. The shape
// mirrors `visibleNavGroups` in layout/nav-registry.ts so AdminShell can
// follow the same render pattern as Sidebar.
export function groupedAdminNav(): Array<{ group: AdminGroup; items: AdminNavItem[] }> {
  return ADMIN_GROUP_ORDER.map((group) => ({
    group,
    items: ADMIN_NAV.filter((item) => item.group === group),
  })).filter(({ items }) => items.length > 0);
}
