// Added (TMAIL-400): AdminShell renders every registry entry as a left-rail
// link, grouped by group header, with the NavLink pointing at /admin/<id>.
// Smoke-level — the per-manager content rendering is covered by each
// manager's own test file and by the E2E walk in admin-shell-extended-flow.
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import { AdminShell } from './AdminShell';
import {
  ADMIN_GROUP_LABELS,
  ADMIN_GROUP_ORDER,
  ADMIN_NAV,
} from './admin-nav-registry';

function renderShellAt(initialPath = '/admin/feature-flags') {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route path="/admin" element={<AdminShell />}>
          {/* Stub child route so AdminShell's <Outlet /> doesn't crash —
              we're only asserting the left rail here. */}
          <Route path=":id" element={<div data-testid="admin-outlet" />} />
        </Route>
      </Routes>
    </MemoryRouter>,
  );
}

describe('AdminShell (TMAIL-400)', () => {
  it('renders the brand + back-to-mailbox link', () => {
    renderShellAt();
    expect(screen.getByText('TASMail Admin')).toBeInTheDocument();
    expect(screen.getByText(/Back to mailbox/i)).toBeInTheDocument();
  });

  it('renders one NavLink per registry entry', () => {
    const { container } = renderShellAt();
    const links = container.querySelectorAll('a.admin-shell__nav-item');
    expect(links.length).toBe(ADMIN_NAV.length);
  });

  it('every group with at least one entry renders a group header', () => {
    const { container } = renderShellAt();
    for (const group of ADMIN_GROUP_ORDER) {
      const hasEntry = ADMIN_NAV.some((item) => item.group === group);
      if (!hasEntry) continue;
      const groupEl = container.querySelector(`[data-testid="admin-shell-group-${group}"]`);
      expect(groupEl, `group ${group}`).not.toBeNull();
      expect(groupEl?.textContent).toContain(ADMIN_GROUP_LABELS[group]);
    }
  });

  it('every NavLink targets /admin/<id>', () => {
    renderShellAt();
    for (const item of ADMIN_NAV) {
      const link = screen.getByTestId(`admin-nav-${item.id}`);
      expect(link.getAttribute('href')).toBe(`/admin/${item.id}`);
      expect(link.textContent).toContain(item.label);
    }
  });

  it('marks the active route with admin-shell__nav-item--active', () => {
    renderShellAt('/admin/saml');
    const active = screen.getByTestId('admin-nav-saml');
    expect(active.className).toContain('admin-shell__nav-item--active');
    const inactive = screen.getByTestId('admin-nav-feature-flags');
    expect(inactive.className).not.toContain('admin-shell__nav-item--active');
  });

  it('renders the <Outlet /> for the matched route', () => {
    renderShellAt('/admin/billing');
    expect(screen.getByTestId('admin-outlet')).toBeInTheDocument();
  });
});
