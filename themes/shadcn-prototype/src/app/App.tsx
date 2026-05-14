import { useEffect, useState } from 'react';
import { RouterProvider } from 'react-router';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { router } from '@/app/routes';
import { apiClient } from '@/api/client';

// TMAIL-220: hand the access_token written by the classic SPA into our
// apiClient on mount. If there is no token, bounce to /login (the classic
// SPA's login page lives at the root, OUTSIDE /modern/, so a full-page
// navigation is the right move). Once login completes the user can re-open
// /modern/ and the token will be available again.
function AuthGate({ children }: { children: React.ReactNode }) {
  const [ready, setReady] = useState(false);
  useEffect(() => {
    const token = localStorage.getItem('access_token');
    if (!token) {
      // Send the user back to the classic login. Preserve a return path so
      // post-login they can re-enter the modern UI if we wire it later.
      window.location.href = '/login?next=/modern/';
      return;
    }
    apiClient.setToken(token);
    setReady(true);
  }, []);
  if (!ready) {
    return (
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '100vh', fontFamily: 'system-ui' }}>
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