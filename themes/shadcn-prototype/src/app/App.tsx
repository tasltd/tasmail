import { useEffect, useState } from 'react';
import { RouterProvider } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { router, PUBLIC_PATHS } from '@/app/routes';
import { apiClient } from '@/api/client';

// TMAIL-220 / TMAIL-327: hand the access_token written by the classic SPA
// (or by the Modern UI's own native /#/login screen) into apiClient on
// mount.
//
// Token lookup order:
//   1. localStorage  — set by login() / signup() when "remember me" is on
//                       (default), also where the classic SPA writes tokens.
//   2. sessionStorage — set by LoginPage when "remember me" is off so the
//                       session ends with the browser session.
//
// If no token is found AND the current hash route isn't one of the public
// auth paths (/login, /signup, /forgot-password), bounce to the Modern UI's
// own login screen. We stay inside /modern/ — no more full-page hop to the
// classic SPA.
function readStoredToken(): string | null {
  return (
    localStorage.getItem('access_token') ||
    sessionStorage.getItem('access_token')
  );
}

function currentHashPath(): string {
  // HashRouter encodes the route in window.location.hash as e.g. "#/login".
  // Strip the leading "#" so we can compare against PUBLIC_PATHS.
  const raw = window.location.hash || '#/';
  const noHash = raw.startsWith('#') ? raw.slice(1) : raw;
  // Drop any query string so /login?next=/foo still matches.
  const [path] = noHash.split('?');
  return path || '/';
}

function AuthGate({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const token = readStoredToken();
    if (token) {
      apiClient.setToken(token);
      setReady(true);
      return;
    }

    // No token — only gate non-public routes. Public auth routes still
    // need to render so unauthenticated visitors can sign in / sign up.
    const path = currentHashPath();
    if ((PUBLIC_PATHS as readonly string[]).includes(path)) {
      setReady(true);
      return;
    }

    // Preserve a return path so post-login they land where they started.
    const nextPath = encodeURIComponent(path);
    window.location.hash = `#/login?next=${nextPath}`;
    setReady(true);
  }, []);

  if (!ready) {
    return (
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          minHeight: '100vh',
          fontFamily: 'system-ui',
        }}
      >
        Loading…
      </div>
    );
  }
  return <>{children}</>;
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: 1, refetchOnWindowFocus: false },
  },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthGate>
        <RouterProvider router={router} />
      </AuthGate>
    </QueryClientProvider>
  );
}
