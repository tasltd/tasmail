import { useState, useCallback } from 'react';
import { Search, Menu, LogOut, Moon, Sun } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useMailStore } from '../../stores/mailStore';

interface TopBarProps {
  onLogout: () => void;
}

export function TopBar({ onLogout }: TopBarProps) {
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const theme = useUiStore((s) => s.theme);
  const toggleTheme = useUiStore((s) => s.toggleTheme);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const [inputValue, setInputValue] = useState('');

  const handleSearch = useCallback(
    (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault();
      if (inputValue.trim().length >= 2) {
        setSearchQuery(inputValue.trim());
      }
    },
    [inputValue, setSearchQuery],
  );

  return (
    <header className="topbar">
      <button className="btn btn--icon" onClick={toggleSidebar}>
        <Menu size={20} />
      </button>

      <form className="topbar__search" onSubmit={handleSearch}>
        <Search size={18} />
        <input
          type="text"
          placeholder="Search emails..."
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
        />
      </form>

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
