import { useEffect } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BrowserRouter, Routes, Route, Navigate, useNavigate } from 'react-router-dom';
import { useAuth } from './hooks/useAuth';
import { useUiStore } from './stores/uiStore';
import { AppShell } from './components/layout/AppShell';
import { LoginPage } from './components/auth/LoginPage';
import { SignupPage } from './components/auth/SignupPage';
import { OnboardingWizard } from './components/onboarding/OnboardingWizard';
import { LandingPage } from './components/landing/LandingPage';
import { PricingPage } from './components/landing/PricingPage';
import { FeatureFlagsManager } from './components/admin/FeatureFlagsManager';
import { QuoteRequestsManager } from './components/admin/QuoteRequestsManager';
import { UsageBillingPage } from './components/billing/UsageBillingPage';
import { ErrorBoundary } from './components/shared/ErrorBoundary';
import './App.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

// Added: Route guard — bounces unauthenticated visitors to the login page.
function RequireAuth({ children }: { children: React.ReactElement }) {
  const { isAuthenticated, isLoading } = useAuth();
  if (isLoading) return <div className="app-loading">Loading...</div>;
  if (!isAuthenticated) return <Navigate to="/login" replace />;
  return children;
}

// Added: Wraps LoginPage so a successful login navigates to /app (the mailbox).
function LoginRoute() {
  const { isAuthenticated, isLoading, login } = useAuth();
  const navigate = useNavigate();
  if (isLoading) return <div className="app-loading">Loading...</div>;
  if (isAuthenticated) return <Navigate to="/app" replace />;
  return (
    <LoginPage
      onLogin={async (username, password) => {
        await login({ username, password });
        navigate('/app', { replace: true });
      }}
    />
  );
}

// Added: The authenticated mailbox app. Logout returns the user to the public landing page.
function AppRoute() {
  const { logout } = useAuth();
  const navigate = useNavigate();
  return (
    <AppShell
      onLogout={async () => {
        await logout();
        navigate('/', { replace: true });
      }}
    />
  );
}

function AppContent() {
  const theme = useUiStore((s) => s.theme);

  // Added: Apply theme attribute to document for CSS dark mode
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  return (
    <BrowserRouter>
      <Routes>
        {/* Public landing page at the root */}
        <Route path="/" element={<LandingPage />} />
        {/* TMAIL-174: dedicated pricing detail page (calculator + FAQ). */}
        <Route path="/pricing" element={<PricingPage />} />
        {/* Public login page */}
        <Route path="/login" element={<LoginRoute />} />
        {/* Public BYOK signup page */}
        <Route path="/signup" element={<SignupPage />} />
        {/* Onboarding wizard — runs after signup OR any time the user has no IMAP/SMTP config yet. */}
        <Route
          path="/onboarding"
          element={
            <RequireAuth>
              <OnboardingWizard />
            </RequireAuth>
          }
        />
        {/* Authenticated mailbox app at /app and any sub-path. AppShell does its own internal routing. */}
        <Route
          path="/app/*"
          element={
            <RequireAuth>
              <AppRoute />
            </RequireAuth>
          }
        />
        {/* TMAIL-166: admin dashboard for runtime feature toggles. Auth required; role gating TBD. */}
        <Route
          path="/admin/feature-flags"
          element={
            <RequireAuth>
              <FeatureFlagsManager />
            </RequireAuth>
          }
        />
        {/* TMAIL-185: admin dashboard for the enterprise quote-request inbox. */}
        <Route
          path="/admin/quote-requests"
          element={
            <RequireAuth>
              <QuoteRequestsManager />
            </RequireAuth>
          }
        />
        {/* TMAIL-179: in-app usage & billing dashboard for the BYOK plan. */}
        <Route
          path="/billing"
          element={
            <RequireAuth>
              <UsageBillingPage />
            </RequireAuth>
          }
        />
        {/* Catch-all: send unknown URLs back to the landing page */}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <AppContent />
      </QueryClientProvider>
    </ErrorBoundary>
  );
}
