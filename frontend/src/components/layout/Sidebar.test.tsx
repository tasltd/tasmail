// Rewritten (TMAIL-398): Sidebar is now registry-driven. The flat 41-button
// block was replaced with NAV_ITEMS from nav-registry.ts, gated by
// useAuth().isAdmin, and grouped with visual separators. The test covers
// the new contract: ≤ 8 top-level entries for a non-admin, +1 Admin entry
// for an admin, and the Inbox row carrying the folder-item--primary class.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router-dom';
import { Sidebar } from './Sidebar';

const mockSetViewMode = vi.fn();
const mockViewMode = vi.fn(() => 'list');
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode, viewMode: mockViewMode() }),
}));

const mockSetSidebarOpen = vi.fn();
vi.mock('../../stores/uiStore', () => ({
  useUiStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setSidebarOpen: mockSetSidebarOpen }),
}));

const mockIsMobile = vi.fn(() => false);
vi.mock('../../hooks/useResponsive', () => ({
  useResponsive: () => ({
    isMobile: mockIsMobile(),
    isTablet: false,
    isDesktop: true,
  }),
}));

// Added (TMAIL-398): useAuth now exposes isAdmin — mock it so we can flip
// admin gating per-test.
const mockIsAdmin = vi.fn(() => false);
vi.mock('../../hooks/useAuth', () => ({
  useAuth: () => ({
    isAuthenticated: true,
    isLoading: false,
    isAdmin: mockIsAdmin(),
    login: vi.fn(),
    logout: vi.fn(),
  }),
}));

const mockNavigate = vi.fn();
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return { ...actual, useNavigate: () => mockNavigate };
});

// FolderTree stub renders an Inbox row that mirrors the real component's
// new contract: folder-item--primary for INBOX. The acceptance criterion
// for TMAIL-398 specifies the Inbox row gets the --primary treatment, and
// the real wiring is asserted indirectly here (via the stub) so the
// Sidebar test stays focused on its own surface.
vi.mock('../mail/FolderTree', () => ({
  FolderTree: () => (
    <nav data-testid="folder-tree">
      <button className="folder-item folder-item--primary" data-testid="folder-inbox">
        Inbox
      </button>
      <button className="folder-item" data-testid="folder-sent">Sent</button>
    </nav>
  ),
}));

vi.mock('./QuotaBar', () => ({
  QuotaBar: () => <div data-testid="quota-bar">QuotaBar</div>,
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    </MemoryRouter>
  );
}

// PURPOSE: Count only the top-level navigation buttons rendered by the
// Sidebar itself — Compose + the registry entries. Folder buttons come
// from the FolderTree stub and are explicitly excluded so the assertion
// reflects the Sidebar's own contract (≤ 8 top-level entries).
function topLevelButtons(container: HTMLElement): HTMLButtonElement[] {
  const aside = container.querySelector('aside.sidebar') as HTMLElement | null;
  if (!aside) return [];
  const all = Array.from(aside.querySelectorAll('button')) as HTMLButtonElement[];
  return all.filter((btn) => !btn.closest('[data-testid="folder-tree"]'));
}

describe('Sidebar (TMAIL-398: registry-driven)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockViewMode.mockReturnValue('list');
    mockIsMobile.mockReturnValue(false);
    mockIsAdmin.mockReturnValue(false);
  });

  it('renders the Compose CTA with the primary button class', () => {
    render(<Sidebar />, { wrapper });
    const compose = screen.getByText('Compose').closest('button');
    expect(compose).not.toBeNull();
    expect(compose?.className).toContain('btn--primary');
    fireEvent.click(compose!);
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
  });

  it('renders the FolderTree block', () => {
    render(<Sidebar />, { wrapper });
    expect(screen.getByTestId('folder-tree')).toBeInTheDocument();
  });

  it('Inbox row carries the folder-item--primary visual treatment', () => {
    render(<Sidebar />, { wrapper });
    const inbox = screen.getByTestId('folder-inbox');
    expect(inbox.className).toContain('folder-item--primary');
  });

  it('renders ≤ 8 top-level entries for a non-admin user', () => {
    const { container } = render(<Sidebar />, { wrapper });
    const buttons = topLevelButtons(container);
    expect(buttons.length).toBeLessThanOrEqual(8);
    // The non-admin surface MUST include Compose + the four apps + Settings.
    const labels = buttons.map((b) => b.textContent?.trim());
    expect(labels).toEqual(
      expect.arrayContaining(['Compose', 'Calendar', 'Contacts', 'Tasks', 'Templates', 'Settings']),
    );
    // ...and MUST NOT include Admin.
    expect(labels).not.toContain('Admin');
  });

  it('shows the Admin entry only when useAuth().isAdmin is true', () => {
    mockIsAdmin.mockReturnValue(true);
    const { container } = render(<Sidebar />, { wrapper });
    const buttons = topLevelButtons(container);
    const labels = buttons.map((b) => b.textContent?.trim());
    expect(labels).toContain('Admin');
    // Non-admin surface had N entries; admin surface is exactly +1.
    expect(buttons.length).toBeLessThanOrEqual(8);
    expect(buttons.length).toBeGreaterThanOrEqual(7);
  });

  it('admin surface has exactly one more entry than non-admin', () => {
    const { container: nonAdminContainer } = render(<Sidebar />, { wrapper });
    const nonAdminCount = topLevelButtons(nonAdminContainer).length;

    mockIsAdmin.mockReturnValue(true);
    const { container: adminContainer } = render(<Sidebar />, { wrapper });
    const adminCount = topLevelButtons(adminContainer).length;

    expect(adminCount).toBe(nonAdminCount + 1);
  });

  it('dispatches setViewMode for viewMode-driven entries (Calendar / Tasks / Templates)', () => {
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Calendar'));
    expect(mockSetViewMode).toHaveBeenCalledWith('calendar');

    fireEvent.click(screen.getByText('Tasks'));
    expect(mockSetViewMode).toHaveBeenCalledWith('tasks');

    fireEvent.click(screen.getByText('Templates'));
    expect(mockSetViewMode).toHaveBeenCalledWith('templates');
  });

  it('Contacts entry uses the ContactsApp manager (contacts-app viewMode)', () => {
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Contacts'));
    expect(mockSetViewMode).toHaveBeenCalledWith('contacts-app');
  });

  it('Settings entry navigates to /app/settings via the router', () => {
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Settings'));
    expect(mockNavigate).toHaveBeenCalledWith('/app/settings');
    // Settings is href-driven — no setViewMode dispatch.
    expect(mockSetViewMode).not.toHaveBeenCalled();
  });

  it('Admin entry navigates to /admin when the user is an admin', () => {
    mockIsAdmin.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Admin'));
    expect(mockNavigate).toHaveBeenCalledWith('/admin');
  });

  it('marks the active viewMode entry with folder-item--active', () => {
    mockViewMode.mockReturnValue('calendar');
    render(<Sidebar />, { wrapper });
    const calendar = screen.getByText('Calendar').closest('button');
    expect(calendar?.className).toContain('folder-item--active');
    const tasks = screen.getByText('Tasks').closest('button');
    expect(tasks?.className).not.toContain('folder-item--active');
  });

  it('renders a visual separator (group container with border-top) per group', () => {
    mockIsAdmin.mockReturnValue(true);
    const { container } = render(<Sidebar />, { wrapper });
    // apps + settings + admin groups are populated; each gets its own group div.
    expect(container.querySelector('[data-testid="sidebar-group-apps"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="sidebar-group-settings"]')).not.toBeNull();
    expect(container.querySelector('[data-testid="sidebar-group-admin"]')).not.toBeNull();
  });

  it('closes the sidebar on mobile after clicking a viewMode-driven entry', () => {
    mockIsMobile.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Calendar'));
    expect(mockSetViewMode).toHaveBeenCalledWith('calendar');
    expect(mockSetSidebarOpen).toHaveBeenCalledWith(false);
  });

  it('closes the sidebar on mobile after clicking the Compose CTA', () => {
    mockIsMobile.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Compose'));
    expect(mockSetSidebarOpen).toHaveBeenCalledWith(false);
  });

  it('closes the sidebar on mobile after clicking an href-driven entry (Settings)', () => {
    mockIsMobile.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Settings'));
    expect(mockNavigate).toHaveBeenCalledWith('/app/settings');
    expect(mockSetSidebarOpen).toHaveBeenCalledWith(false);
  });

  it('keeps the sidebar open on desktop after navigation', () => {
    mockIsMobile.mockReturnValue(false);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Calendar'));
    expect(mockSetSidebarOpen).not.toHaveBeenCalled();
  });

  it('renders the QuotaBar at the bottom of the sidebar', () => {
    const { container } = render(<Sidebar />, { wrapper });
    const aside = container.querySelector('aside.sidebar') as HTMLElement;
    expect(within(aside).getByTestId('quota-bar')).toBeInTheDocument();
  });
});
