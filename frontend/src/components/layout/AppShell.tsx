import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';
import { MessageList } from '../mail/MessageList';
import { MessageView } from '../mail/MessageView';
import { Composer } from '../mail/Composer';
import { SearchResults } from '../mail/SearchResults';
import { SignatureManager } from '../settings/SignatureManager';
import { ContactManager } from '../settings/ContactManager';
import { TwoFactorManager } from '../settings/TwoFactorManager';
import { VacationResponder } from '../settings/VacationResponder';
import { GroupManager } from '../settings/GroupManager';
import { MigrationManager } from '../settings/MigrationManager';
import { LowBandwidthSettings } from '../settings/LowBandwidthSettings';
import { FilterManager } from '../settings/FilterManager';
// Added: Shared mailbox management component (TMAIL-96)
import { SharedMailboxManager } from '../settings/SharedMailboxManager';
// Added: Email queue management component (TMAIL-58)
import { QueueManager } from '../settings/QueueManager';
// Added: Task/to-do management component (TMAIL-126)
import { TaskManager } from '../settings/TaskManager';
// Added: Webhook management component (TMAIL-131)
import { WebhookManager } from '../settings/WebhookManager';
// Added: Branding management component (TMAIL-111)
import { BrandingManager } from '../settings/BrandingManager';
// Added: Retention policy and legal hold management component (TMAIL-109)
import { RetentionManager } from '../settings/RetentionManager';
// Added: Custom hostname management component (TMAIL-112)
import { HostnameManager } from '../settings/HostnameManager';
// Added: Shared file management component for large file sharing (TMAIL-138)
import { SharedFileManager } from '../settings/SharedFileManager';
// Added: Bulk user import management component (TMAIL-136)
import { BulkImportManager } from '../settings/BulkImportManager';
// Added: Chat integration management component (TMAIL-129)
import { ChatIntegrationManager } from '../settings/ChatIntegrationManager';
// Added: Calendar/meeting scheduling management component (TMAIL-127)
import { CalendarManager } from '../settings/CalendarManager';
// Added: LDAP/AD directory sync management component (TMAIL-100)
import { LdapManager } from '../settings/LdapManager';
// Added: AI configuration management component for BYOK AI integration (TMAIL-105)
import { AiConfigManager } from '../settings/AiConfigManager';
// Added: SAML 2.0 SSO configuration management component (TMAIL-101)
import { SamlManager } from '../settings/SamlManager';
// Added: OIDC identity provider management component (TMAIL-99)
import { OidcManager } from '../settings/OidcManager';
// Added: eDiscovery search management component (TMAIL-137)
import { EdiscoveryManager } from '../settings/EdiscoveryManager';
// Added: DLP rule management component for Data Loss Prevention (TMAIL-108)
import { DlpManager } from '../settings/DlpManager';
// Added: DANE/TLSA policy and verification management component (TMAIL-125)
import { DaneManager } from '../settings/DaneManager';
// Added: BYO-SMTP configuration management component (TMAIL-48)
import { SmtpConfigManager } from '../settings/SmtpConfigManager';
// Added: Plugin management component for extensible plugin architecture (TMAIL-132)
import { PluginManager } from '../settings/PluginManager';
// Added: Contacts App management component with groups, import/export, merge (TMAIL-119)
import { ContactsApp } from '../settings/ContactsApp';
// Added: POP3 configuration management component for Dovecot POP3 access (TMAIL-133)
import { Pop3ConfigManager } from '../settings/Pop3ConfigManager';
// Added: Email archive management component for Piler integration (TMAIL-107)
import { ArchiveManager } from '../settings/ArchiveManager';
// Added: ActiveSync device management component for TMAIL-130
import { ActiveSyncManager } from '../settings/ActiveSyncManager';
// Added: Ollama local LLM management component (TMAIL-102)
import { OllamaManager } from '../settings/OllamaManager';
// Added: CalDAV/CardDAV configuration management component (TMAIL-117)
import { DavConfigManager } from '../settings/DavConfigManager';
import { useMailStore } from '../../stores/mailStore';
import { useUiStore } from '../../stores/uiStore';
// Added: Keyboard shortcuts hook and help dialog for TMAIL-121
import { useKeyboardShortcuts } from '../../hooks/useKeyboardShortcuts';
import { KeyboardShortcutHelp } from '../shared/KeyboardShortcutHelp';

interface AppShellProps {
  onLogout: () => void;
}

export function AppShell({ onLogout }: AppShellProps) {
  const viewMode = useMailStore((s) => s.viewMode);
  const sidebarOpen = useUiStore((s) => s.sidebarOpen);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  // Added: Gmail-like keyboard shortcuts (TMAIL-121)
  const { showHelp, setShowHelp } = useKeyboardShortcuts();

  return (
    <div className={`app-shell ${sidebarOpen ? '' : 'app-shell--sidebar-collapsed'}`}>
      <TopBar onLogout={onLogout} />
      <div className="app-shell__body">
        {sidebarOpen && (
          <>
            <div className="sidebar-overlay" onClick={toggleSidebar} />
            <Sidebar />
          </>
        )}
        <main className="app-shell__content">
          {viewMode === 'list' && <MessageList />}
          {viewMode === 'reader' && <MessageView />}
          {viewMode === 'compose' && <Composer />}
          {viewMode === 'search' && <SearchResults />}
          {viewMode === 'signatures' && <SignatureManager />}
          {viewMode === 'contacts' && <ContactManager />}
          {viewMode === 'security' && <TwoFactorManager />}
          {viewMode === 'vacation' && <VacationResponder />}
          {viewMode === 'groups' && <GroupManager />}
          {viewMode === 'migration' && <MigrationManager />}
          {viewMode === 'bandwidth' && <LowBandwidthSettings />}
          {viewMode === 'filters' && <FilterManager />}
          {/* Added: Shared mailbox ACL management view (TMAIL-96) */}
          {viewMode === 'shared' && <SharedMailboxManager />}
          {/* Added: Email queue management view (TMAIL-58) */}
          {viewMode === 'queue' && <QueueManager />}
          {/* Added: Task/to-do management view (TMAIL-126) */}
          {viewMode === 'tasks' && <TaskManager />}
          {/* Added: Webhook management view (TMAIL-131) */}
          {viewMode === 'webhooks' && <WebhookManager />}
          {/* Added: Branding management view (TMAIL-111) */}
          {viewMode === 'branding' && <BrandingManager />}
          {/* Added: Retention policy and legal hold management view (TMAIL-109) */}
          {viewMode === 'retention' && <RetentionManager />}
          {/* Added: Custom hostname management view (TMAIL-112) */}
          {viewMode === 'hostnames' && <HostnameManager />}
          {/* Added: Shared file management view (TMAIL-138) */}
          {viewMode === 'shared-files' && <SharedFileManager />}
          {/* Added: Bulk user import management view (TMAIL-136) */}
          {viewMode === 'bulk-import' && <BulkImportManager />}
          {/* Added: Chat integration management view (TMAIL-129) */}
          {viewMode === 'chat' && <ChatIntegrationManager />}
          {/* Added: Calendar/meeting scheduling view (TMAIL-127) */}
          {viewMode === 'calendar' && <CalendarManager />}
          {/* Added: LDAP/AD directory sync management view (TMAIL-100) */}
          {viewMode === 'ldap' && <LdapManager />}
          {/* Added: AI configuration management view (TMAIL-105) */}
          {viewMode === 'ai-config' && <AiConfigManager />}
          {/* Added: SAML SSO configuration management view (TMAIL-101) */}
          {viewMode === 'saml' && <SamlManager />}
          {/* Added: OIDC provider management view (TMAIL-99) */}
          {viewMode === 'oidc' && <OidcManager />}
          {/* Added: eDiscovery search management view (TMAIL-137) */}
          {viewMode === 'ediscovery' && <EdiscoveryManager />}
          {/* Added: DLP rule management view (TMAIL-108) */}
          {viewMode === 'dlp' && <DlpManager />}
          {/* Added: DANE/TLSA policy and verification management view (TMAIL-125) */}
          {viewMode === 'dane' && <DaneManager />}
          {/* Added: BYO-SMTP configuration management view (TMAIL-48) */}
          {viewMode === 'smtp-config' && <SmtpConfigManager />}
          {/* Added: Plugin management view (TMAIL-132) */}
          {viewMode === 'plugins' && <PluginManager />}
          {/* Added: Contacts App management view (TMAIL-119) */}
          {viewMode === 'contacts-app' && <ContactsApp />}
          {/* Added: POP3 configuration management view (TMAIL-133) */}
          {viewMode === 'pop3' && <Pop3ConfigManager />}
          {/* Added: Email archive management view (TMAIL-107) */}
          {viewMode === 'archive' && <ArchiveManager />}
          {/* Added: ActiveSync device management view (TMAIL-130) */}
          {viewMode === 'activesync' && <ActiveSyncManager />}
          {/* Added: Ollama local LLM management view (TMAIL-102) */}
          {viewMode === 'ollama' && <OllamaManager />}
          {/* Added: CalDAV/CardDAV configuration management view (TMAIL-117) */}
          {viewMode === 'dav-config' && <DavConfigManager />}
        </main>
      </div>
      {/* Added: Keyboard shortcut help dialog, toggled by '?' key */}
      {showHelp && <KeyboardShortcutHelp onClose={() => setShowHelp(false)} />}
    </div>
  );
}
