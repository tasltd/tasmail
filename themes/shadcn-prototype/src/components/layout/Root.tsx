import { useState, useEffect } from 'react';
import { Outlet } from 'react-router';
import { Navbar } from '@/components/layout/Navbar';

export function Root() {
  const [darkMode, setDarkMode] = useState(() => {
    // Check system preference on initial load
    if (typeof window !== 'undefined') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches;
    }
    return false;
  });

  useEffect(() => {
    // Update document class when dark mode changes
    if (darkMode) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [darkMode]);

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100">
      <Navbar darkMode={darkMode} onToggleDarkMode={() => setDarkMode(!darkMode)} />
      <div className="flex-1 overflow-hidden">
        <Outlet />
      </div>
    </div>
  );
}
