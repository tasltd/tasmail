import { createBrowserRouter } from "react-router";
import { Root } from '@/components/layout/Root';
import { EmailClient } from '@/features/email/EmailClient';
import { AdminDashboard } from '@/features/admin/AdminDashboard';
import { CalendarView } from '@/features/calendar/CalendarView';

export const router = createBrowserRouter([
  {
    path: "/",
    Component: Root,
    children: [
      { index: true, Component: EmailClient },
      { path: "admin", Component: AdminDashboard },
      { path: "calendar", Component: CalendarView },
    ],
  },
]);
