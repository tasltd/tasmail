import { Mail, Moon, Sun, Settings, Search } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

interface NavbarProps {
  darkMode: boolean;
  onToggleDarkMode: () => void;
}

export function Navbar({ darkMode, onToggleDarkMode }: NavbarProps) {
  return (
    <nav className="border-b border-zinc-200 dark:border-zinc-800 bg-white dark:bg-zinc-950 px-3 sm:px-4 py-2 sm:py-0 flex flex-col sm:flex-row sm:h-14 sm:items-center gap-2 sm:gap-4">
      {/* Top row: logo + actions */}
      <div className="flex items-center justify-between sm:contents">
        <div className="flex items-center gap-2 shrink-0">
          <Mail className="size-5 sm:size-6 text-blue-600 dark:text-blue-400" />
          <span className="font-semibold text-base sm:text-lg">Rust Mail</span>
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
          <Button variant="ghost" size="icon" title="Settings">
            <Settings className="size-4 sm:size-5" />
          </Button>
        </div>
      </div>

      {/* Search bar — full width below on mobile, inline on desktop */}
      <div className="relative w-full sm:flex-1 sm:max-w-2xl">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-zinc-400" />
        <Input
          type="text"
          placeholder="Search mail..."
          className="w-full pl-10 bg-zinc-50 dark:bg-zinc-900 border-zinc-200 dark:border-zinc-800 text-sm"
        />
      </div>
    </nav>
  );
}
