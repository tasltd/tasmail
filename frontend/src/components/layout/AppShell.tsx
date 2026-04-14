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
        </main>
      </div>
      {/* Added: Keyboard shortcut help dialog, toggled by '?' key */}
      {showHelp && <KeyboardShortcutHelp onClose={() => setShowHelp(false)} />}
    </div>
  );
}
