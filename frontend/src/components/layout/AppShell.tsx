import { Sidebar } from './Sidebar';
import { TopBar } from './TopBar';
import { MessageList } from '../mail/MessageList';
import { MessageView } from '../mail/MessageView';
import { Composer } from '../mail/Composer';
import { useMailStore } from '../../stores/mailStore';
import { useUiStore } from '../../stores/uiStore';

interface AppShellProps {
  onLogout: () => void;
}

export function AppShell({ onLogout }: AppShellProps) {
  const viewMode = useMailStore((s) => s.viewMode);
  const sidebarOpen = useUiStore((s) => s.sidebarOpen);

  return (
    <div className={`app-shell ${sidebarOpen ? '' : 'app-shell--sidebar-collapsed'}`}>
      <TopBar onLogout={onLogout} />
      <div className="app-shell__body">
        {sidebarOpen && <Sidebar />}
        <main className="app-shell__content">
          {viewMode === 'list' && <MessageList />}
          {viewMode === 'reader' && <MessageView />}
          {viewMode === 'compose' && <Composer />}
        </main>
      </div>
    </div>
  );
}
