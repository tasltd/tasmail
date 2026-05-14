import { useState, useCallback } from 'react';
import { Search, Menu, LogOut, Moon, Sun, WifiOff, SlidersHorizontal, Sparkles, BrainCircuit, Wand2 } from 'lucide-react';
import { useUiStore } from '../../stores/uiStore';
import { useMailStore } from '../../stores/mailStore';
import { useOnlineStatus } from '../../hooks/useOnlineStatus';
// Added: Import AdvancedSearch panel for TMAIL-32
import { AdvancedSearch } from '../mail/AdvancedSearch';
// Added: Import SemanticSearchPanel for TMAIL-106
import { SemanticSearchPanel } from '../mail/SemanticSearchPanel';
// Added: Import NlpSearchPanel for TMAIL-135
import { NlpSearchPanel } from '../mail/NlpSearchPanel';

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
  // Added: Toggle state for semantic search panel (TMAIL-106)
  const [showSemanticSearch, setShowSemanticSearch] = useState(false);
  // Added: Toggle state for NLP search panel (TMAIL-135)
  const [showNlpSearch, setShowNlpSearch] = useState(false);

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
      {/* Changed: Added data-testid for mobile hamburger toggle (TMAIL-33) */}
      <button className="btn btn--icon" onClick={toggleSidebar} data-testid="sidebar-toggle">
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
          {/* Added: Semantic search toggle button for TMAIL-106 */}
          <button
            type="button"
            className={`btn btn--icon topbar__semantic-toggle ${showSemanticSearch ? 'topbar__semantic-toggle--active' : ''}`}
            onClick={() => setShowSemanticSearch((prev) => !prev)}
            title="Toggle semantic search"
            data-testid="semantic-search-toggle"
          >
            <Sparkles size={18} />
          </button>
          {/* Added: NLP search toggle button for TMAIL-135 */}
          <button
            type="button"
            className={`btn btn--icon topbar__nlp-toggle ${showNlpSearch ? 'topbar__nlp-toggle--active' : ''}`}
            onClick={() => setShowNlpSearch((prev) => !prev)}
            title="Toggle AI search"
            data-testid="nlp-search-toggle"
          >
            <BrainCircuit size={18} />
          </button>
        </form>
        {/* Added: Advanced search filter panel */}
        <AdvancedSearch visible={showFilters} />
        {/* Added: Semantic search panel for TMAIL-106 */}
        {showSemanticSearch && <SemanticSearchPanel />}
        {/* Added: NLP search panel for TMAIL-135 */}
        {showNlpSearch && <NlpSearchPanel />}
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
        {/* TMAIL-223: hop to the alt-UI (built bundle in frontend/public/modern/).
            Full-page nav (anchor) so React Router doesn't intercept. The alt-UI's
            AuthGate reads the JWT this SPA already wrote to localStorage. */}
        <a
          className="btn btn--icon"
          href="/modern/index.html"
          title="Try the modern UI"
          aria-label="Try the modern UI"
        >
          <Wand2 size={18} />
        </a>
        <button className="btn btn--icon" onClick={onLogout} title="Logout">
          <LogOut size={18} />
        </button>
      </div>
    </header>
  );
}
