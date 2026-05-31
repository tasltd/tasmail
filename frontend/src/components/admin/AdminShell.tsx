// TMAIL-197: shared chrome for /admin/* pages.
// Changed (TMAIL-400): the inline 8-entry NAV array is replaced by the
// data-driven admin-nav-registry.ts. The left rail now groups all 26
// admin managers under their group headers (System / Tenant / Identity /
// Compliance / Mail / Integrations / Billing) so the surface stays
// scannable as we add more operator tools. RequireAdmin sits one level
// up in the route tree; App.tsx's /admin Routes block iterates the same
// registry to register the per-id Routes.
import { NavLink, Outlet } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import {
  ADMIN_GROUP_LABELS,
  groupedAdminNav,
  type AdminNavItem,
} from './admin-nav-registry';
import './AdminShell.css';

export function AdminShell() {
  const groups = groupedAdminNav();
  return (
    <div className="admin-shell">
      <aside className="admin-shell__sidebar">
        <div className="admin-shell__brand">
          <strong>TASMail Admin</strong>
        </div>
        <a href="/app" className="admin-shell__back">
          <ArrowLeft size={14} /> Back to mailbox
        </a>
        <nav className="admin-shell__nav" data-testid="admin-shell-nav">
          {groups.map(({ group, items }) => (
            <div
              key={group}
              className={`admin-shell__group admin-shell__group--${group}`}
              data-testid={`admin-shell-group-${group}`}
            >
              <div className="admin-shell__group-label">{ADMIN_GROUP_LABELS[group]}</div>
              {items.map((item) => (
                <AdminNavEntry key={item.id} item={item} />
              ))}
            </div>
          ))}
        </nav>
      </aside>
      <main className="admin-shell__content">
        <Outlet />
      </main>
    </div>
  );
}

function AdminNavEntry({ item }: { item: AdminNavItem }) {
  const Icon = item.icon;
  return (
    <NavLink
      to={`/admin/${item.id}`}
      data-testid={`admin-nav-${item.id}`}
      className={({ isActive }) =>
        `admin-shell__nav-item ${isActive ? 'admin-shell__nav-item--active' : ''}`
      }
    >
      <Icon size={18} />
      <span>{item.label}</span>
    </NavLink>
  );
}

// TMAIL-198..203 placeholder. Each follow-up ticket replaces the body with a
// real manager component but keeps the file path and route stable. Kept
// after the TMAIL-400 refactor so any future stubbed entry can reuse it.
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
