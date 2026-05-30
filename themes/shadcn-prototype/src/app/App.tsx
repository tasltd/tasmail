import { useEffect, useState } from 'react';
import { RouterProvider } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { router, PUBLIC_PATHS } from '@/app/routes';
import { apiClient, TOKEN_CHANGED_EVENT } from '@/api/client';
import { useWebSocket } from '@/hooks/useWebSocket';

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
  // Added (TMAIL-328): track the active JWT so the WS hook below can open
  // /ws?token=... once auth resolves. We keep the source of truth on the
  // singleton apiClient, but expose it as React state so the hook re-mounts
  // when it changes (login → logout flips this to null and the hook tears
  // down the socket; login → swap-account flips it to a new token and the
  // hook reconnects with the new JWT).
  const [accessToken, setAccessToken] = useState<string | null>(null);

  useEffect(() => {
    const token = readStoredToken();
    if (token) {
      apiClient.setToken(token);
      setAccessToken(token);
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

  // Added (TMAIL-328): keep `accessToken` in lockstep with apiClient over
  // the session lifetime. The initial-mount effect above only fires once,
  // but the user may log in, refresh, or sign out without remounting
  // AuthGate (LoginPage uses React Router navigate, not a full reload).
  //
  // Two listeners drive the resync:
  //   1. TOKEN_CHANGED_EVENT — fired by apiClient.setToken on same-tab
  //      login / refresh / logout.
  //   2. storage — fired by the browser on cross-tab localStorage changes
  //      so a login in another tab still spins up the WS here.
  useEffect(() => {
    const sync = () => setAccessToken(readStoredToken());
    const onCustom = (e: Event) => {
      const detail = (e as CustomEvent<{ token: string | null }>).detail;
      setAccessToken(detail?.token ?? readStoredToken());
    };
    const onStorage = (e: StorageEvent) => {
      if (e.key === 'access_token' || e.key === null) sync();
    };
    window.addEventListener(TOKEN_CHANGED_EVENT, onCustom);
    window.addEventListener('storage', onStorage);
    return () => {
      window.removeEventListener(TOKEN_CHANGED_EVENT, onCustom);
      window.removeEventListener('storage', onStorage);
    };
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
  return (
    <>
      {/* TMAIL-328: real-time push subscription. Mounted INSIDE AuthGate so
          it only opens after a token is in hand. WsBridge holds the
          useWebSocket call so toggling auth state cleanly mounts /
          unmounts the socket along with the rest of the authed UI. */}
      <WsBridge token={accessToken} />
      {children}
    </>
  );
}

// TMAIL-328: Tiny presence-component that owns the /ws subscription so the
// hook lives inside the QueryClientProvider and can invalidate the same
// query cache the rest of the app reads from. Renders nothing — the only
// side-effect is the WebSocket lifecycle managed by useWebSocket().
function WsBridge({ token }: { token: string | null }) {
  useWebSocket({ token, subscribeFolders: ['INBOX'] });
  return null;
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
