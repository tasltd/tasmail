import { useState, useCallback } from 'react';
import { Search, Menu, LogOut, Moon, Sun, WifiOff, SlidersHorizontal } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useMailStore } from '../../stores/mailStore';
import { useOnlineStatus } from '../../hooks/useOnlineStatus';
// Added: Import AdvancedSearch panel for TMAIL-32
import { AdvancedSearch } from '../mail/AdvancedSearch';

interface TopBarProps {
  onLogout: () => void;
}

export function TopBar({ onLogout }: TopBarProps) {
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);
  const theme = useUiStore((s) => s.theme);
  const toggleTheme = useUiStore((s) => s.toggleTheme);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const isOnline = useOnlineStatus();
  const [inputValue, setInputValue] = useState('');
  // Added: Toggle state for advanced search filter panel
  const [showFilters, setShowFilters] = useState(false);

  const handleSearch = useCallback(
    (e: React.FormEvent) => {
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

      <div className="topbar__search-wrapper">
        <form className="topbar__search" onSubmit={handleSearch}>
          <Search size={18} />
          <input
            type="text"
            placeholder="Search emails..."
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
          />
          {/* Added: Filter toggle button for TMAIL-32 advanced search */}
          <button
            type="button"
            className={`btn btn--icon topbar__filter-toggle ${showFilters ? 'topbar__filter-toggle--active' : ''}`}
            onClick={() => setShowFilters((prev) => !prev)}
            title="Toggle advanced filters"
            data-testid="filter-toggle"
          >
            <SlidersHorizontal size={18} />
          </button>
        </form>
        {/* Added: Advanced search filter panel */}
        <AdvancedSearch visible={showFilters} />
      </div>

      <div className="topbar__actions">
        {/* Added: Offline indicator for PWA support */}
        {!isOnline && (
          <span className="topbar__offline" title="You are offline — changes will sync when reconnected">
            <WifiOff size={18} /> Offline
          </span>
        )}
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
