// TMAIL-222: HashRouter so internal navigation stays inside /modern/index.html
// (e.g. /modern/index.html#/admin) instead of asking the parent Vite host to
// resolve /modern/admin — which would fall through to the production SPA.
import { createHashRouter } from "react-router";
import { Root } from '@/components/layout/Root';
import { EmailClient } from '@/features/email/EmailClient';
import { AdminDashboard } from '@/features/admin/AdminDashboard';
import { CalendarView } from '@/features/calendar/CalendarView';
import { SearchResultsPage } from '@/features/email/SearchResultsPage';

export const router = createHashRouter([
  {
    path: "/",
    Component: Root,
    children: [
      { index: true, Component: EmailClient },
      { path: "admin", Component: AdminDashboard },
      { path: "calendar", Component: CalendarView },
      // Added (TMAIL-322): Navbar search bar submits to /#/search?q=...
      { path: "search", Component: SearchResultsPage },
    ],
  },
]);
