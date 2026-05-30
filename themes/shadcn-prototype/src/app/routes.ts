// TMAIL-222: HashRouter so internal navigation stays inside /modern/index.html
// (e.g. /modern/admin) instead of asking the parent Vite host to
// resolve /modern/admin — which would fall through to the production SPA.
//
// TMAIL-327: /login, /signup, /forgot-password are PUBLIC routes that render
// outside the Root shell. They are also outside AuthGate (see App.tsx) so
// unauthenticated visitors can land here directly.
import { createHashRouter, redirect } from "react-router";
import { Root } from '@/components/layout/Root';
import { EmailClient } from '@/features/email/EmailClient';
import { AdminDashboard } from '@/features/admin/AdminDashboard';
import { CalendarView } from '@/features/calendar/CalendarView';
import { SearchResultsPage } from '@/features/email/SearchResultsPage';
import { SettingsPage } from '@/features/settings/SettingsPage';
import { DEFAULT_SETTINGS_TAB } from '@/features/settings/tabs';
import { LoginPage } from '@/features/auth/LoginPage';
import { SignupPage } from '@/features/auth/SignupPage';
import { ForgotPasswordPage } from '@/features/auth/ForgotPasswordPage';

// Added (TMAIL-327): the set of routes that do NOT require a valid JWT.
// AuthGate consults this list to decide whether to render the children
// directly or bounce to /#/login.
export const PUBLIC_PATHS = ['/login', '/signup', '/forgot-password'] as const;

export const router = createHashRouter([
  // Public auth routes — no Root shell, no AuthGate
  { path: "/login", Component: LoginPage },
  { path: "/signup", Component: SignupPage },
  { path: "/forgot-password", Component: ForgotPasswordPage },
  {
    path: "/",
    Component: Root,
    children: [
      { index: true, Component: EmailClient },
      { path: "admin", Component: AdminDashboard },
      { path: "calendar", Component: CalendarView },
      // Added (TMAIL-322): Navbar search bar submits to /#/search?q=...
      { path: "search", Component: SearchResultsPage },
      // Added (TMAIL-323): /settings shell with side-tab layout. Bare
      // /settings redirects to the default tab so the active pane is
      // always represented in the URL (deep-link + reload friendly). A
      // loader-based redirect keeps this file JSX-free (it's a .ts module).
      {
        path: "settings",
        children: [
          {
            index: true,
            loader: () => redirect(`/settings/${DEFAULT_SETTINGS_TAB.slug}`),
          },
          { path: ":tab", Component: SettingsPage },
        ],
      },
    ],
  },
]);
