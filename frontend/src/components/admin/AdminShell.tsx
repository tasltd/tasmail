// TMAIL-197: shared chrome for /admin/* pages.
//
// Sidebar + outlet so every admin manager (existing FeatureFlagsManager,
// QuoteRequestsManager + the six TMAIL-198..203 stubs) renders inside the
// same layout. RequireAdmin sits one level up in the route tree.
import { NavLink, Outlet } from 'react-router-dom';
import {
  ToggleRight,
  Inbox,
  ScrollText,
  Database,
  Globe,
  CreditCard,
  Users,
  Activity,
  ArrowLeft,
} from 'lucide-react';
import './AdminShell.css';

interface NavEntry {
  to: string;
  label: string;
  icon: React.ReactElement;
}

const NAV: NavEntry[] = [
  { to: '/admin/feature-flags', label: 'Feature flags', icon: <ToggleRight size={18} /> },
  { to: '/admin/quote-requests', label: 'Quote requests', icon: <Inbox size={18} /> },
  { to: '/admin/audit-log', label: 'Audit log', icon: <ScrollText size={18} /> },
  { to: '/admin/cache', label: 'Cache', icon: <Database size={18} /> },
  { to: '/admin/domains', label: 'Domains', icon: <Globe size={18} /> },
  { to: '/admin/payment-providers', label: 'Payment providers', icon: <CreditCard size={18} /> },
  { to: '/admin/users', label: 'Users', icon: <Users size={18} /> },
  { to: '/admin/warmup', label: 'IP warm-up', icon: <Activity size={18} /> },
];

export function AdminShell() {
  return (
    <div className="admin-shell">
      <aside className="admin-shell__sidebar">
        <div className="admin-shell__brand">
          <strong>TASMail Admin</strong>
        </div>
        <a href="/app" className="admin-shell__back">
          <ArrowLeft size={14} /> Back to mailbox
        </a>
        <nav className="admin-shell__nav">
          {NAV.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                `admin-shell__nav-item ${isActive ? 'admin-shell__nav-item--active' : ''}`
              }
            >
              {item.icon}
              <span>{item.label}</span>
            </NavLink>
          ))}
        </nav>
      </aside>
      <main className="admin-shell__content">
        <Outlet />
      </main>
    </div>
  );
}

// TMAIL-198..203 placeholder. Each follow-up ticket replaces the body with a
// real manager component but keeps the file path and route stable.
export function AdminPlaceholder({
  title,
  ticket,
  description,
}: {
  title: string;
  ticket: string;
  description: string;
}) {
  return (
    <div className="admin-placeholder">
      <h1>{title}</h1>
      <p className="admin-placeholder__desc">{description}</p>
      <p className="admin-placeholder__ticket">
        Tracked in <strong>{ticket}</strong>. The page renders here once that ticket ships.
      </p>
    </div>
  );
}
