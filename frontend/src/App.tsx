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
// Added: TMAIL-197 — admin shell + role guard + placeholders for the
// six follow-up admin pages (TMAIL-198..203).
import { AdminShell, AdminPlaceholder } from './components/admin/AdminShell';
import { RequireAdmin } from './components/admin/RequireAdmin';
import { AuditLogManager } from './components/admin/AuditLogManager';
import { CacheManager } from './components/admin/CacheManager';
import { DomainsManager } from './components/admin/DomainsManager';
import { PaymentProvidersManager } from './components/admin/PaymentProvidersManager';
import { UsersManager } from './components/admin/UsersManager';
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
        {/* TMAIL-197: every /admin/* page mounts inside AdminShell behind the
            RequireAdmin gate. Existing FeatureFlagsManager and QuoteRequestsManager
            move under here; placeholders cover TMAIL-198..203 until each manager
            ships. */}
        <Route
          path="/admin"
          element={
            <RequireAuth>
              <RequireAdmin>
                <AdminShell />
              </RequireAdmin>
            </RequireAuth>
          }
        >
          <Route index element={<Navigate to="feature-flags" replace />} />
          <Route path="feature-flags" element={<FeatureFlagsManager />} />
          <Route path="quote-requests" element={<QuoteRequestsManager />} />
          <Route path="audit-log" element={<AuditLogManager />} />
          <Route path="cache" element={<CacheManager />} />
          <Route path="domains" element={<DomainsManager />} />
          <Route path="payment-providers" element={<PaymentProvidersManager />} />
          <Route path="users" element={<UsersManager />} />
          <Route
            path="warmup"
            element={
              <AdminPlaceholder
                title="IP warm-up"
                ticket="TMAIL-203"
                description="Visualise the 8-week IP warm-up schedule and current send-volume vs target from /api/admin/warmup/{status,schedule}, with a Start button."
              />
            }
          />
        </Route>
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
