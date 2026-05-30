// TMAIL-323: alt-UI /settings shell. Side-tab layout matching the spec
// (Profile · Identities · Signatures · Vacation · Filters · MFA · Theme ·
// IMAP/SMTP). The tab list is driven by `SETTINGS_TABS` so adding a new
// pane is a one-line registry edit — see ./tabs.ts.
//
// The active tab is encoded in the URL as `/settings/:tab` so a deep link
// (e.g. /modern/index.html#/settings/mfa) lands on the right pane and a
// page reload preserves the user's selection. When no tab is in the URL
// (`/settings` exactly) we redirect to the default tab inside the route
// definitions (see app/routes.ts) — keeps this component pure.
//
// Sub-pane implementations land in P1 tasks. Until then every tab renders
// `SettingsTabPlaceholder` which describes what's coming. The actual
// `<Outlet />` is wired so swapping a placeholder for a real pane is a
// route-only change.
import { Link, NavLink, useParams } from 'react-router';
import { ArrowLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/components/ui/utils';
import {
  SETTINGS_TABS,
  DEFAULT_SETTINGS_TAB,
  findTabBySlug,
  type SettingsTab,
} from '@/features/settings/tabs';
import { SettingsTabPlaceholder } from '@/features/settings/SettingsTabPlaceholder';

export function SettingsPage() {
  // `tab` is the dynamic segment from /settings/:tab. When the user lands
  // on /settings exactly, the route config redirects them to the default
  // tab so this hook is never undefined in practice — but the fallback
  // keeps the component safe in isolation (e.g. component tests).
  const { tab: slug } = useParams<{ tab?: string }>();
  const activeTab: SettingsTab = findTabBySlug(slug) ?? DEFAULT_SETTINGS_TAB;

  return (
    <div
      data-testid="settings-page"
      className="h-full w-full flex flex-col sm:flex-row bg-white dark:bg-zinc-950 overflow-hidden"
    >
      {/* Side-nav: vertical on desktop, horizontal scroll on mobile */}
      <aside
        data-testid="settings-sidenav"
        aria-label="Settings sections"
        className="border-b sm:border-b-0 sm:border-r border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900/40 sm:w-64 sm:shrink-0 sm:overflow-y-auto"
      >
        <div className="hidden sm:flex items-center gap-2 px-4 py-3 border-b border-zinc-200 dark:border-zinc-800">
          <Link to="/" aria-label="Back to inbox">
            <Button variant="ghost" size="icon">
              <ArrowLeft className="size-4" />
            </Button>
          </Link>
          <span className="font-semibold text-sm">Settings</span>
        </div>
        <nav
          className={cn(
            'flex sm:flex-col gap-1 p-2 overflow-x-auto sm:overflow-x-visible',
          )}
        >
          {SETTINGS_TABS.map((t) => (
            <SettingsSideNavLink key={t.slug} tab={t} />
          ))}
        </nav>
      </aside>

      {/* Active pane. TMAIL-331: registry-driven — if the tab defines a
          concrete `component`, render that; otherwise fall back to the
          placeholder. This lets P1 panes ship one at a time without
          touching this file or the route table. */}
      <main className="flex-1 min-w-0 overflow-y-auto">
        {activeTab.component ? (
          <activeTab.component />
        ) : (
          <SettingsTabPlaceholder tab={activeTab} />
        )}
      </main>
    </div>
  );
}

interface SettingsSideNavLinkProps {
  tab: SettingsTab;
}

function SettingsSideNavLink({ tab }: SettingsSideNavLinkProps) {
  const Icon = tab.icon;
  return (
    <NavLink
      to={`/settings/${tab.slug}`}
      data-testid={tab.testId}
      className={({ isActive }) =>
        cn(
          'flex items-center gap-2 rounded-md px-3 py-2 text-sm whitespace-nowrap transition-colors',
          isActive
            ? 'bg-blue-600 text-white hover:bg-blue-600'
            : 'text-zinc-700 dark:text-zinc-300 hover:bg-zinc-100 dark:hover:bg-zinc-800',
        )
      }
    >
      <Icon className="size-4 shrink-0" aria-hidden="true" />
      <span>{tab.label}</span>
    </NavLink>
  );
}
