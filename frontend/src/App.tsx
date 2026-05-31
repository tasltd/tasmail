import { lazy, Suspense, useEffect } from 'react';
import type { ReactNode } from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { BrowserRouter, Routes, Route, Navigate, useNavigate } from 'react-router-dom';
import { useAuth } from './hooks/useAuth';
import { useBranding } from './hooks/useBranding';
import { useUiStore } from './stores/uiStore';
import { AppShell } from './components/layout/AppShell';
import { LoginPage } from './components/auth/LoginPage';
import { LandingPage } from './components/landing/LandingPage';
import { ErrorBoundary } from './components/shared/ErrorBoundary';
import './App.css';

// Changed (TMAIL-259): auxiliary routes (signup, onboarding, pricing, admin,
// billing, public booking) are now lazy-loaded. Landing + login stay eager —
// they are the public entry points and adding a Suspense round-trip there
// would hurt first-paint for the marketing surface. Admin shell + its eight
// managers all share one chunk after manualChunks vendor splitting.
const SignupPage = lazy(() => import('./components/auth/SignupPage').then((m) => ({ default: m.SignupPage })));
const OnboardingWizard = lazy(() => import('./components/onboarding/OnboardingWizard').then((m) => ({ default: m.OnboardingWizard })));
const PricingPage = lazy(() => import('./components/landing/PricingPage').then((m) => ({ default: m.PricingPage })));
const FeatureFlagsManager = lazy(() => import('./components/admin/FeatureFlagsManager').then((m) => ({ default: m.FeatureFlagsManager })));
const QuoteRequestsManager = lazy(() => import('./components/admin/QuoteRequestsManager').then((m) => ({ default: m.QuoteRequestsManager })));
const AdminShell = lazy(() => import('./components/admin/AdminShell').then((m) => ({ default: m.AdminShell })));
const RequireAdmin = lazy(() => import('./components/admin/RequireAdmin').then((m) => ({ default: m.RequireAdmin })));
const AuditLogManager = lazy(() => import('./components/admin/AuditLogManager').then((m) => ({ default: m.AuditLogManager })));
const CacheManager = lazy(() => import('./components/admin/CacheManager').then((m) => ({ default: m.CacheManager })));
const DomainsManager = lazy(() => import('./components/admin/DomainsManager').then((m) => ({ default: m.DomainsManager })));
const PaymentProvidersManager = lazy(() => import('./components/admin/PaymentProvidersManager').then((m) => ({ default: m.PaymentProvidersManager })));
const UsersManager = lazy(() => import('./components/admin/UsersManager').then((m) => ({ default: m.UsersManager })));
const WarmupManager = lazy(() => import('./components/admin/WarmupManager').then((m) => ({ default: m.WarmupManager })));
const UsageBillingPage = lazy(() => import('./components/billing/UsageBillingPage').then((m) => ({ default: m.UsageBillingPage })));
const BookingPage = lazy(() => import('./components/booking/BookingPage').then((m) => ({ default: m.BookingPage })));
// Added (TMAIL-399): SettingsHub mounts under /app/settings/* and replaces the
// scattered viewMode-driven managers (TwoFactor, Signatures, Filters, etc.).
// Lazy-loaded so the initial mailbox bundle stays slim — the hub itself then
// lazy-loads each section component out of settings-hub-registry.ts.
const SettingsHub = lazy(() => import('./components/settings/SettingsHub').then((m) => ({ default: m.SettingsHub })));

// Added (TMAIL-259): shared Suspense fallback for the route-level lazy splits.
function RouteLoading() {
  return <div className="app-loading">Loading…</div>;
}

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
// Changed (TMAIL-399): accept an optional `content` override so /app/settings/*
// can reuse AppShell's chrome (Sidebar + TopBar + offline banner) while
// rendering SettingsHub in the main area instead of the viewMode ladder.
function AppRoute({ content }: { content?: ReactNode } = {}) {
  const { logout } = useAuth();
  const navigate = useNavigate();
  return (
    <AppShell
      onLogout={async () => {
        await logout();
        navigate('/', { replace: true });
      }}
      content={content}
    />
  );
}

function AppContent() {
  const theme = useUiStore((s) => s.theme);

  // Added: Apply theme attribute to document for CSS dark mode
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  }, [theme]);

  // Added: TMAIL-111 — fetch /api/branding and apply CSS vars, title, favicon,
  // and custom_css to the running document. Public endpoint, runs for both
  // signed-in and signed-out routes (landing/login also pick up the brand).
  useBranding();

  return (
    <BrowserRouter>
      <Suspense fallback={<RouteLoading />}>
      <Routes>
        {/* Public landing page at the root */}
        <Route path="/" element={<LandingPage />} />
        {/* TMAIL-174: dedicated pricing detail page (calculator + FAQ). */}
        <Route path="/pricing" element={<PricingPage />} />
        {/* Public login page */}
        <Route path="/login" element={<LoginRoute />} />
        {/* Public BYOK signup page */}
        <Route path="/signup" element={<SignupPage />} />
        {/* TMAIL-269: public scheduling page for external participants — no auth required */}
        <Route path="/book/:token" element={<BookingPage />} />
        {/* Onboarding wizard — runs after signup OR any time the user has no IMAP/SMTP config yet. */}
        <Route
          path="/onboarding"
          element={
            <RequireAuth>
              <OnboardingWizard />
            </RequireAuth>
          }
        />
        {/* TMAIL-399: Gmail-style Settings hub. More specific than /app/*, so
            React Router ranks it ahead. AppShell keeps the chrome; SettingsHub
            takes over the main content area. */}
        <Route
          path="/app/settings/*"
          element={
            <RequireAuth>
              <AppRoute content={<SettingsHub />} />
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
          <Route path="warmup" element={<WarmupManager />} />
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
        {/* TMAIL-224: anything starting with /modern is the alt-UI bundle —
            full-page navigate so React Router gets out of the way and Vite
            serves the static index.html from frontend/public/modern/. */}
        <Route path="/modern/*" element={<ModernUiBounce />} />
        {/* Catch-all: send unknown URLs back to the landing page */}
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
      </Suspense>
    </BrowserRouter>
  );
}

// TMAIL-224 — full-page navigation out of the React tree to the alt-UI
// static bundle. Used both for users who paste /modern/ in the URL bar and
// for users who land here after an internal redirect.
function ModernUiBounce() {
  useEffect(() => {
    window.location.replace('/modern/index.html');
  }, []);
  return <div className="app-loading">Opening modern UI…</div>;
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
