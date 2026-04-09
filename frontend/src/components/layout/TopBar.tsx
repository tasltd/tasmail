import { Search, Menu, LogOut, Moon, Sun } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';

interface TopBarProps {
  onLogout: () => void;
}

export function TopBar({ onLogout }: TopBarProps) {
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const theme = useUiStore((s) => s.theme);
  const toggleTheme = useUiStore((s) => s.toggleTheme);

  return (
    <header className="topbar">
      <button className="btn btn--icon" onClick={toggleSidebar}>
        <Menu size={20} />
      </button>

      <div className="topbar__search">
        <Search size={18} />
        <input type="text" placeholder="Search emails..." />
      </div>

      <div className="topbar__actions">
        <button className="btn btn--icon" onClick={toggleTheme} title="Toggle theme">
          {theme === 'light' ? <Moon size={18} /> : <Sun size={18} />}
        </button>
        <button className="btn btn--icon" onClick={onLogout} title="Logout">
          <LogOut size={18} />
        </button>
      </div>
    </header>
  );
}
