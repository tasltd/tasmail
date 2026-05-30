// TMAIL-322: search bar is now a controlled <form>. Submit (Enter or clicking
// the Search icon) navigates immediately to /#/search?q=<encoded>; while the
// user is typing a 400 ms debounce pushes the same URL so SearchResultsPage
// stays in sync without the user pressing Enter.
//
// The current URL's `?q=` seeds the input on mount so a page reload or a
// deep-linked SearchResultsPage starts with the input pre-filled — keeps the
// header in sync with the route.
import { useEffect, useRef, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router';
import { Mail, Moon, Sun, Settings, Search } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface NavbarProps {
  darkMode: boolean;
  onToggleDarkMode: () => void;
}

// Added (TMAIL-322): debounce window in ms. 400 ms is the same value the
// classic SPA uses for its global search input — keep them in sync so the
// two interfaces feel identical when toggling between them.
const SEARCH_DEBOUNCE_MS = 400;

export function Navbar({ darkMode, onToggleDarkMode }: NavbarProps) {
  const navigate = useNavigate();
  const location = useLocation();

  // Seed the controlled value from the current URL `?q=`. With HashRouter
  // the query string lives in `location.search` (react-router lifts the
  // post-`?` portion of the hash into `location.search` for us).
  const [value, setValue] = useState<string>(() => {
    if (typeof window === 'undefined') return '';
    const params = new URLSearchParams(location.search);
    return params.get('q') ?? '';
  });

  // Keep `value` in sync if the URL changes externally (e.g. user clicks a
  // sidebar link that lands them on /#/search?q=…). Important so the header
  // doesn't show a stale query after a back/forward navigation.
  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const q = params.get('q') ?? '';
    setValue((prev) => (prev === q ? prev : q));
  }, [location.search]);

  // Debounce the typed value → /#/search?q=… navigation. Cleared on every
  // keystroke so we only fire after the user pauses typing. Skips empty
  // strings (no point hitting the backend for nothing) and skips when the
  // debounced value already matches the URL (idempotent push).
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      // Empty input shouldn't auto-navigate — leaves the user wherever they
      // were. The form's submit handler (below) handles "clear and Enter"
      // explicitly if we ever want it.
      return;
    }
    const currentQ = new URLSearchParams(location.search).get('q') ?? '';
    const onSearchRoute = location.pathname === '/search';
    if (onSearchRoute && currentQ === trimmed) return;

    debounceRef.current = setTimeout(() => {
      navigate(`/search?q=${encodeURIComponent(trimmed)}`);
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [value, location.pathname, location.search, navigate]);

  const submitSearch = () => {
    const trimmed = value.trim();
    if (trimmed.length === 0) return;
    // Submit is the synchronous path — cancel any pending debounce so we
    // don't race against ourselves, then navigate immediately.
    if (debounceRef.current) clearTimeout(debounceRef.current);
    navigate(`/search?q=${encodeURIComponent(trimmed)}`);
  };

  return (
    <nav className="border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 px-3 sm:px-4 py-2 sm:py-0 flex flex-col sm:flex-row sm:h-14 sm:items-center gap-2 sm:gap-4">
      {/* Top row: logo + actions */}
      <div className="flex items-center justify-between sm:contents">
        <div className="flex items-center gap-2 shrink-0">
          <Mail className="size-5 sm:size-6 text-blue-600 dark:text-blue-400" />
          <span className="font-semibold text-base sm:text-lg">TASMail</span>
          <span className="text-xs text-zinc-500 hidden sm:inline">· Modern UI</span>
          <a href="/app" className="text-xs text-blue-600 hover:underline ml-2 hidden sm:inline" title="Go back to the classic dashboard">← Classic</a>
        </div>

        <div className="flex items-center gap-1 shrink-0 sm:ml-auto">
          <Button
            variant="ghost"
            size="icon"
            onClick={onToggleDarkMode}
            title={darkMode ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            {darkMode ? <Sun className="size-4 sm:size-5" /> : <Moon className="size-4 sm:size-5" />}
          </Button>
          {/* Added (TMAIL-323): Settings icon now routes to the /settings
              shell. Bare /settings redirects to the default tab so the
              active pane is always pinned in the URL. */}
          <Link to="/settings" aria-label="Open settings" data-testid="navbar-settings-link">
            <Button variant="ghost" size="icon" title="Settings">
              <Settings className="size-4 sm:size-5" />
            </Button>
          </Link>
        </div>
      </div>

      {/* Search bar — full width below on mobile, inline on desktop */}
      <form
        role="search"
        onSubmit={(e) => {
          e.preventDefault();
          submitSearch();
        }}
        className="relative w-full sm:flex-1 sm:max-w-2xl"
      >
        <label htmlFor="modern-ui-search" className="sr-only">
          Search mail
        </label>
        <button
          type="submit"
          aria-label="Submit search"
          className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-200 focus:outline-none"
        >
          <Search className="size-4" />
        </button>
        <Input
          id="modern-ui-search"
          name="q"
          type="search"
          autoComplete="off"
          placeholder="Search mail..."
          value={value}
          onChange={(e) => setValue(e.target.value)}
          data-testid="modern-ui-search-input"
          className="w-full pl-10 bg-zinc-50 dark:bg-zinc-900 border-zinc-200 dark:border-zinc-800 text-sm"
        />
      </form>
    </nav>
  );
}
