import { lazy, Suspense } from 'react';
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

// Changed (TMAIL-259): Every Settings manager + Composer + SearchResults moved
// to React.lazy() so the initial mailbox bundle ships only the list/reader
// flow. Each manager becomes its own on-demand chunk, keyed off viewMode.
// The Composer also includes the @tiptap/* vendor chunk; deferring it cuts
// ~150 kB raw from the entry. See docs/assessments/frontend-bundle-2026-05.md.
const Composer = lazy(() => import('../mail/Composer').then((m) => ({ default: m.Composer })));
const SearchResults = lazy(() => import('../mail/SearchResults').then((m) => ({ default: m.SearchResults })));
const SignatureManager = lazy(() => import('../settings/SignatureManager').then((m) => ({ default: m.SignatureManager })));
const ContactManager = lazy(() => import('../settings/ContactManager').then((m) => ({ default: m.ContactManager })));
const TwoFactorManager = lazy(() => import('../settings/TwoFactorManager').then((m) => ({ default: m.TwoFactorManager })));
const VacationResponder = lazy(() => import('../settings/VacationResponder').then((m) => ({ default: m.VacationResponder })));
const GroupManager = lazy(() => import('../settings/GroupManager').then((m) => ({ default: m.GroupManager })));
const MigrationManager = lazy(() => import('../settings/MigrationManager').then((m) => ({ default: m.MigrationManager })));
const LowBandwidthSettings = lazy(() => import('../settings/LowBandwidthSettings').then((m) => ({ default: m.LowBandwidthSettings })));
const FilterManager = lazy(() => import('../settings/FilterManager').then((m) => ({ default: m.FilterManager })));
const SharedMailboxManager = lazy(() => import('../settings/SharedMailboxManager').then((m) => ({ default: m.SharedMailboxManager })));
const QueueManager = lazy(() => import('../settings/QueueManager').then((m) => ({ default: m.QueueManager })));
const TaskManager = lazy(() => import('../settings/TaskManager').then((m) => ({ default: m.TaskManager })));
const WebhookManager = lazy(() => import('../settings/WebhookManager').then((m) => ({ default: m.WebhookManager })));
const BrandingManager = lazy(() => import('../settings/BrandingManager').then((m) => ({ default: m.BrandingManager })));
const RetentionManager = lazy(() => import('../settings/RetentionManager').then((m) => ({ default: m.RetentionManager })));
const HostnameManager = lazy(() => import('../settings/HostnameManager').then((m) => ({ default: m.HostnameManager })));
const SharedFileManager = lazy(() => import('../settings/SharedFileManager').then((m) => ({ default: m.SharedFileManager })));
const BulkImportManager = lazy(() => import('../settings/BulkImportManager').then((m) => ({ default: m.BulkImportManager })));
const ChatIntegrationManager = lazy(() => import('../settings/ChatIntegrationManager').then((m) => ({ default: m.ChatIntegrationManager })));
const CalendarManager = lazy(() => import('../settings/CalendarManager').then((m) => ({ default: m.CalendarManager })));
const LdapManager = lazy(() => import('../settings/LdapManager').then((m) => ({ default: m.LdapManager })));
const AiConfigManager = lazy(() => import('../settings/AiConfigManager').then((m) => ({ default: m.AiConfigManager })));
const SamlManager = lazy(() => import('../settings/SamlManager').then((m) => ({ default: m.SamlManager })));
const OidcManager = lazy(() => import('../settings/OidcManager').then((m) => ({ default: m.OidcManager })));
const EdiscoveryManager = lazy(() => import('../settings/EdiscoveryManager').then((m) => ({ default: m.EdiscoveryManager })));
const DlpManager = lazy(() => import('../settings/DlpManager').then((m) => ({ default: m.DlpManager })));
const DaneManager = lazy(() => import('../settings/DaneManager').then((m) => ({ default: m.DaneManager })));
const SmtpConfigManager = lazy(() => import('../settings/SmtpConfigManager').then((m) => ({ default: m.SmtpConfigManager })));
const PluginManager = lazy(() => import('../settings/PluginManager').then((m) => ({ default: m.PluginManager })));
const ContactsApp = lazy(() => import('../settings/ContactsApp').then((m) => ({ default: m.ContactsApp })));
const Pop3ConfigManager = lazy(() => import('../settings/Pop3ConfigManager').then((m) => ({ default: m.Pop3ConfigManager })));
const ArchiveManager = lazy(() => import('../settings/ArchiveManager').then((m) => ({ default: m.ArchiveManager })));
const ActiveSyncManager = lazy(() => import('../settings/ActiveSyncManager').then((m) => ({ default: m.ActiveSyncManager })));
const OllamaManager = lazy(() => import('../settings/OllamaManager').then((m) => ({ default: m.OllamaManager })));
const DavConfigManager = lazy(() => import('../settings/DavConfigManager').then((m) => ({ default: m.DavConfigManager })));
const SpamFilterManager = lazy(() => import('../settings/SpamFilterManager').then((m) => ({ default: m.SpamFilterManager })));
const BillingManager = lazy(() => import('../settings/BillingManager').then((m) => ({ default: m.BillingManager })));
const DeliverabilityReport = lazy(() => import('../settings/DeliverabilityReport').then((m) => ({ default: m.DeliverabilityReport })));
const PushDevicesManager = lazy(() => import('../settings/PushDevicesManager').then((m) => ({ default: m.PushDevicesManager })));

interface AppShellProps {
  onLogout: () => void;
}

// Added (TMAIL-259): single fallback used by every lazy Suspense boundary in
// the shell. Plain text avoids pulling additional dependencies into the
// initial chunk just for a spinner.
function ViewLoading() {
  return <div className="app-shell__view-loading">Loading…</div>;
}

export function AppShell({ onLogout }: AppShellProps) {
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
          {/* Eager — the list + reader is the entry surface and must paint
              without an extra round trip. Everything else is gated behind
              Suspense so its code only loads when the user selects it. */}
          {viewMode === 'list' && <MessageList />}
          {viewMode === 'reader' && <MessageView />}
          {viewMode !== 'list' && viewMode !== 'reader' && (
            <Suspense fallback={<ViewLoading />}>
              {viewMode === 'compose' && <Composer />}
              {viewMode === 'search' && <SearchResults />}
              {viewMode === 'signatures' && <SignatureManager />}
              {viewMode === 'contacts' && <ContactManager />}
              {viewMode === 'security' && <TwoFactorManager />}
              {viewMode === 'push-devices' && <PushDevicesManager />}
              {viewMode === 'vacation' && <VacationResponder />}
              {viewMode === 'groups' && <GroupManager />}
              {viewMode === 'migration' && <MigrationManager />}
              {viewMode === 'bandwidth' && <LowBandwidthSettings />}
              {viewMode === 'filters' && <FilterManager />}
              {viewMode === 'shared' && <SharedMailboxManager />}
              {viewMode === 'queue' && <QueueManager />}
              {viewMode === 'tasks' && <TaskManager />}
              {viewMode === 'webhooks' && <WebhookManager />}
              {viewMode === 'branding' && <BrandingManager />}
              {viewMode === 'retention' && <RetentionManager />}
              {viewMode === 'hostnames' && <HostnameManager />}
              {viewMode === 'shared-files' && <SharedFileManager />}
              {viewMode === 'bulk-import' && <BulkImportManager />}
              {viewMode === 'chat' && <ChatIntegrationManager />}
              {viewMode === 'calendar' && <CalendarManager />}
              {viewMode === 'ldap' && <LdapManager />}
              {viewMode === 'ai-config' && <AiConfigManager />}
              {viewMode === 'saml' && <SamlManager />}
              {viewMode === 'oidc' && <OidcManager />}
              {viewMode === 'ediscovery' && <EdiscoveryManager />}
              {viewMode === 'dlp' && <DlpManager />}
              {viewMode === 'dane' && <DaneManager />}
              {viewMode === 'smtp-config' && <SmtpConfigManager />}
              {viewMode === 'plugins' && <PluginManager />}
              {viewMode === 'contacts-app' && <ContactsApp />}
              {viewMode === 'pop3' && <Pop3ConfigManager />}
              {viewMode === 'archive' && <ArchiveManager />}
              {viewMode === 'activesync' && <ActiveSyncManager />}
              {viewMode === 'ollama' && <OllamaManager />}
              {viewMode === 'dav-config' && <DavConfigManager />}
              {viewMode === 'spam' && <SpamFilterManager />}
              {viewMode === 'billing' && <BillingManager />}
              {viewMode === 'deliverability' && <DeliverabilityReport />}
            </Suspense>
          )}
        </main>
      </div>
      {/* Added: Keyboard shortcut help dialog, toggled by '?' key */}
      {showHelp && <KeyboardShortcutHelp onClose={() => setShowHelp(false)} />}
    </div>
  );
}
