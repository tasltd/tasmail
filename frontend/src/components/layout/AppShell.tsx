import { lazy, Suspense } from 'react';
import type { ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';
import { MessageList } from '../mail/MessageList';
import { MessageView } from '../mail/MessageView';
import { useMailStore } from '../../stores/mailStore';
import { useUiStore } from '../../stores/uiStore';
// Added: Keyboard shortcuts hook and help dialog for TMAIL-121
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { KeyboardShortcutHelp } from '../shared/KeyboardShortcutHelp';
// Added: Responsive hook for mobile layout handling (TMAIL-33)
import { useResponsive } from '../../hooks/useResponsive';
// Added (TMAIL-32): two-way sync between search state and the URL query string.
import { useSearchUrlSync } from '../../hooks/useSearchUrlSync';
// Added (TMAIL-88): "Pending sync: N actions" banner for offline queue visibility.
import { PendingSyncBanner } from '../shared/PendingSyncBanner';
// Added (TMAIL-401): lazy-loaded first-login product tour. Renders nothing
// for users who have already dismissed it, so the import + chunk is cheap
// after the initial visit.
const FirstLoginTour = lazy(() =>
  import('../onboarding/FirstLoginTour').then((m) => ({ default: m.FirstLoginTour })),
);

// Changed (TMAIL-259): Every Settings manager + Composer + SearchResults moved
// to React.lazy() so the initial mailbox bundle ships only the list/reader
// flow. Each manager becomes its own on-demand chunk, keyed off viewMode.
// The Composer also includes the @tiptap/* vendor chunk; deferring it cuts
// ~150 kB raw from the entry. See docs/assessments/frontend-bundle-2026-05.md.
//
// Changed (TMAIL-399): Managers that moved to SettingsHub (Two-Factor, Push
// Devices, Signatures, Vacation, Filters, Spam, SMTP, POP3, DAV, Migration,
// Shared Files, Groups, AI Config, Ollama, Low Bandwidth) no longer get
// lazy-imported here — the hub owns their lifecycle now via
// settings-hub-registry.ts. The viewMode keys for those panes still exist
// in the union for store-API back-compat, but AppShell's viewMode ladder
// no longer branches on them.
const Composer = lazy(() => import('../mail/Composer').then((m) => ({ default: m.Composer })));
const SearchResults = lazy(() => import('../mail/SearchResults').then((m) => ({ default: m.SearchResults })));
const ContactManager = lazy(() => import('../settings/ContactManager').then((m) => ({ default: m.ContactManager })));
const TemplateManager = lazy(() => import('../settings/TemplateManager').then((m) => ({ default: m.TemplateManager })));
const SharedMailboxManager = lazy(() => import('../settings/SharedMailboxManager').then((m) => ({ default: m.SharedMailboxManager })));
const QueueManager = lazy(() => import('../settings/QueueManager').then((m) => ({ default: m.QueueManager })));
const TaskManager = lazy(() => import('../settings/TaskManager').then((m) => ({ default: m.TaskManager })));
const WebhookManager = lazy(() => import('../settings/WebhookManager').then((m) => ({ default: m.WebhookManager })));
const BrandingManager = lazy(() => import('../settings/BrandingManager').then((m) => ({ default: m.BrandingManager })));
const RetentionManager = lazy(() => import('../settings/RetentionManager').then((m) => ({ default: m.RetentionManager })));
const HostnameManager = lazy(() => import('../settings/HostnameManager').then((m) => ({ default: m.HostnameManager })));
const BulkImportManager = lazy(() => import('../settings/BulkImportManager').then((m) => ({ default: m.BulkImportManager })));
const ChatIntegrationManager = lazy(() => import('../settings/ChatIntegrationManager').then((m) => ({ default: m.ChatIntegrationManager })));
const CalendarManager = lazy(() => import('../settings/CalendarManager').then((m) => ({ default: m.CalendarManager })));
const LdapManager = lazy(() => import('../settings/LdapManager').then((m) => ({ default: m.LdapManager })));
const SamlManager = lazy(() => import('../settings/SamlManager').then((m) => ({ default: m.SamlManager })));
const OidcManager = lazy(() => import('../settings/OidcManager').then((m) => ({ default: m.OidcManager })));
const EdiscoveryManager = lazy(() => import('../settings/EdiscoveryManager').then((m) => ({ default: m.EdiscoveryManager })));
const DlpManager = lazy(() => import('../settings/DlpManager').then((m) => ({ default: m.DlpManager })));
const DaneManager = lazy(() => import('../settings/DaneManager').then((m) => ({ default: m.DaneManager })));
const PluginManager = lazy(() => import('../settings/PluginManager').then((m) => ({ default: m.PluginManager })));
const ContactsApp = lazy(() => import('../settings/ContactsApp').then((m) => ({ default: m.ContactsApp })));
const ArchiveManager = lazy(() => import('../settings/ArchiveManager').then((m) => ({ default: m.ArchiveManager })));
const ActiveSyncManager = lazy(() => import('../settings/ActiveSyncManager').then((m) => ({ default: m.ActiveSyncManager })));
const BillingManager = lazy(() => import('../settings/BillingManager').then((m) => ({ default: m.BillingManager })));
const DeliverabilityReport = lazy(() => import('../settings/DeliverabilityReport').then((m) => ({ default: m.DeliverabilityReport })));

interface AppShellProps {
  onLogout: () => void;
  // Added (TMAIL-399): when set, AppShell renders the override (wrapped in
  // Suspense) instead of the viewMode-driven content. Used by /app/settings/*
  // to mount SettingsHub inside the same chrome.
  content?: ReactNode;
}

// Added (TMAIL-259): single fallback used by every lazy Suspense boundary in
// the shell. Plain text avoids pulling additional dependencies into the
// initial chunk just for a spinner.
function ViewLoading() {
  return <div className="app-shell__view-loading">Loading…</div>;
}

export function AppShell({ onLogout, content }: AppShellProps) {
  const viewMode = useMailStore((s) => s.viewMode);
  const sidebarOpen = useUiStore((s) => s.sidebarOpen);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  // Added: Gmail-like keyboard shortcuts (TMAIL-121)
  const { showHelp, setShowHelp } = useKeyboardShortcuts();
  // Added: Responsive breakpoint detection for mobile layout (TMAIL-33)
  const { isMobile } = useResponsive();
  // Added (TMAIL-32): keep ?q=... and advanced filters in the URL so search is bookmarkable.
  useSearchUrlSync();

  return (
    <div className={`app-shell ${sidebarOpen ? '' : 'app-shell--sidebar-collapsed'}`}>
      <TopBar onLogout={onLogout} />
      {/* Added (TMAIL-88): banner shows queued offline actions; renders null when empty. */}
      <PendingSyncBanner />
      <div className="app-shell__body">
        {/* Changed: Show sidebar overlay only on mobile; desktop always shows sidebar inline (TMAIL-33) */}
        {sidebarOpen && (
          <>
            {isMobile && (
              <div
                className="sidebar-overlay"
                data-testid="sidebar-overlay"
                onClick={toggleSidebar}
              />
            )}
            <Sidebar />
          </>
        )}
        {/* Added: On desktop, always show sidebar even if sidebarOpen is false (TMAIL-33) */}
        {!sidebarOpen && !isMobile && <Sidebar />}
        <main className="app-shell__content">
          {content ? (
            // TMAIL-399: route-level override (SettingsHub). Wrapped in
            // Suspense because the caller passes a lazy component.
            <Suspense fallback={<ViewLoading />}>{content}</Suspense>
          ) : (
            <>
              {/* Eager — the list + reader is the entry surface and must paint
                  without an extra round trip. Everything else is gated behind
                  Suspense so its code only loads when the user selects it. */}
              {viewMode === 'list' && <MessageList />}
              {viewMode === 'reader' && <MessageView />}
              {viewMode !== 'list' && viewMode !== 'reader' && (
                <Suspense fallback={<ViewLoading />}>
                  {viewMode === 'compose' && <Composer />}
                  {viewMode === 'search' && <SearchResults />}
                  {viewMode === 'contacts' && <ContactManager />}
                  {/* TMAIL-286 + TMAIL-399: TemplateManager has both a sidebar
                      app entry (viewMode='templates') AND a hub section under
                      Mail. The sidebar route stays the one-click path. */}
                  {viewMode === 'templates' && <TemplateManager />}
                  {viewMode === 'shared' && <SharedMailboxManager />}
                  {viewMode === 'queue' && <QueueManager />}
                  {viewMode === 'tasks' && <TaskManager />}
                  {viewMode === 'webhooks' && <WebhookManager />}
                  {viewMode === 'branding' && <BrandingManager />}
                  {viewMode === 'retention' && <RetentionManager />}
                  {viewMode === 'hostnames' && <HostnameManager />}
                  {viewMode === 'bulk-import' && <BulkImportManager />}
                  {viewMode === 'chat' && <ChatIntegrationManager />}
                  {viewMode === 'calendar' && <CalendarManager />}
                  {viewMode === 'ldap' && <LdapManager />}
                  {viewMode === 'saml' && <SamlManager />}
                  {viewMode === 'oidc' && <OidcManager />}
                  {viewMode === 'ediscovery' && <EdiscoveryManager />}
                  {viewMode === 'dlp' && <DlpManager />}
                  {viewMode === 'dane' && <DaneManager />}
                  {viewMode === 'plugins' && <PluginManager />}
                  {viewMode === 'contacts-app' && <ContactsApp />}
                  {viewMode === 'archive' && <ArchiveManager />}
                  {viewMode === 'activesync' && <ActiveSyncManager />}
                  {viewMode === 'billing' && <BillingManager />}
                  {viewMode === 'deliverability' && <DeliverabilityReport />}
                </Suspense>
              )}
            </>
          )}
        </main>
      </div>
      {/* Added: Keyboard shortcut help dialog, toggled by '?' key */}
      {showHelp && <KeyboardShortcutHelp onClose={() => setShowHelp(false)} />}
      {/* Added (TMAIL-401): mount the first-login tour once per shell. The
          component handles its own visibility — for users who have already
          dismissed it, it renders null after a single GET. */}
      <Suspense fallback={null}>
        <FirstLoginTour />
      </Suspense>
    </div>
  );
}
