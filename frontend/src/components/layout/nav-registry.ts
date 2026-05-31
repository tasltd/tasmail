// Added (TMAIL-398): registry that drives the new grouped Sidebar.
// Replaces the inline 41-button block in Sidebar.tsx with a data-driven
// config so future agents extend navigation by adding an entry here
// instead of editing the component.
//
// Group order is fixed (mail → apps → settings → admin) so the Sidebar
// renderer can iterate the groups in display order and insert a separator
// between each one without per-item placement logic.
import {
  Calendar,
  Users,
  CheckSquare,
  FileText,
  Settings as SettingsIcon,
  Shield,
  type LucideIcon,
} from 'lucide-react';

// Only the subset of ViewMode values that the registry actually targets.
// SettingsHub + Admin are not viewMode-driven (they navigate via href), so
// they get the discriminant 'settings-hub' / 'admin' instead. Keeping this
// narrower than the full ViewMode union avoids tempting future agents to
// stuff every setting back into the sidebar — adding a NEW entry here is
// a deliberate act, not the path of least resistance.
type NavViewMode = 'calendar' | 'contacts-app' | 'tasks' | 'templates';

export type NavGroup = 'mail' | 'apps' | 'settings' | 'admin';

export interface NavItem {
  key: NavViewMode | 'settings-hub' | 'admin';
  icon: LucideIcon;
  label: string;
  group: NavGroup;
  adminOnly?: boolean;
  // href is set for entries that navigate via the router (Settings opens
  // the SettingsHub page, Admin opens the existing /admin shell). Items
  // without href dispatch setViewMode(key) inside the current AppShell.
  href?: string;
}

// Order matters — this is also the render order within each group.
export const NAV_ITEMS: NavItem[] = [
  { key: 'calendar', icon: Calendar, label: 'Calendar', group: 'apps' },
  { key: 'contacts-app', icon: Users, label: 'Contacts', group: 'apps' },
  { key: 'tasks', icon: CheckSquare, label: 'Tasks', group: 'apps' },
  { key: 'templates', icon: FileText, label: 'Templates', group: 'apps' },
  { key: 'settings-hub', icon: SettingsIcon, label: 'Settings', group: 'settings', href: '/app/settings' },
  { key: 'admin', icon: Shield, label: 'Admin', group: 'admin', adminOnly: true, href: '/admin' },
];

// Fixed group render order. The Sidebar iterates this list and renders a
// separator between each non-empty group.
export const NAV_GROUP_ORDER: NavGroup[] = ['mail', 'apps', 'settings', 'admin'];

// PURPOSE: Filter the registry for the current user's role and bucket
// entries by group ready for rendering.
// CONSTRAINTS: Returns groups in NAV_GROUP_ORDER and drops empty groups so
// the renderer never emits an orphan separator.
export function visibleNavGroups(isAdmin: boolean): Array<{ group: NavGroup; items: NavItem[] }> {
  const allowed = NAV_ITEMS.filter((item) => !item.adminOnly || isAdmin);
  return NAV_GROUP_ORDER
    .map((group) => ({ group, items: allowed.filter((item) => item.group === group) }))
    .filter(({ items }) => items.length > 0);
}
