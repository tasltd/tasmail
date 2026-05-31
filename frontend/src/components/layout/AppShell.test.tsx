import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
// Added (TMAIL-32): AppShell now uses useSearchUrlSync, which needs a Router context.
import { MemoryRouter } from 'react-router-dom';
import { AppShell } from './AppShell';

// Added: Mock stores
const mockViewMode = vi.fn(() => 'list');
const mockSidebarOpen = vi.fn(() => true);
const mockToggleSidebar = vi.fn();

vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ viewMode: mockViewMode() }),
}));

vi.mock('../../stores/uiStore', () => ({
  useUiStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ sidebarOpen: mockSidebarOpen(), toggleSidebar: mockToggleSidebar }),
}));

// Added: Mock useResponsive — defaults to mobile for overlay tests (TMAIL-33)
const mockIsMobile = vi.fn(() => true);
vi.mock('../../hooks/useResponsive', () => ({
  useResponsive: () => ({
    isMobile: mockIsMobile(),
    isTablet: false,
    isDesktop: !mockIsMobile(),
  }),
}));

// Added: Mock all child components
vi.mock('./TopBar', () => ({
  TopBar: ({ onLogout }: { onLogout: () => void }) => (
    <div data-testid="top-bar">
      <button onClick={onLogout}>Logout</button>
    </div>
  ),
}));
vi.mock('./Sidebar', () => ({
  Sidebar: () => <div data-testid="sidebar">Sidebar</div>,
}));
vi.mock('../mail/MessageList', () => ({
  MessageList: () => <div data-testid="message-list">MessageList</div>,
}));
vi.mock('../mail/MessageView', () => ({
  MessageView: () => <div data-testid="message-view">MessageView</div>,
}));
vi.mock('../mail/Composer', () => ({
  Composer: () => <div data-testid="composer">Composer</div>,
}));
vi.mock('../mail/SearchResults', () => ({
  SearchResults: () => <div data-testid="search-results">SearchResults</div>,
}));
vi.mock('../settings/ContactManager', () => ({
  ContactManager: () => <div data-testid="contact-manager">ContactManager</div>,
}));
// TMAIL-399: TwoFactor, Signatures, Vacation, Groups, Migration, LowBandwidth
// no longer mount via AppShell's viewMode ladder — they live in SettingsHub
// under /app/settings/* and are tested in SettingsHub.test.tsx.

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <MemoryRouter>
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    </MemoryRouter>
  );
}

describe('AppShell', () => {
  const mockLogout = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockViewMode.mockReturnValue('list');
    mockSidebarOpen.mockReturnValue(true);
    mockIsMobile.mockReturnValue(true);
  });

  it('renders TopBar with onLogout prop', () => {
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.getByTestId('top-bar')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Logout'));
    expect(mockLogout).toHaveBeenCalled();
  });

  it('renders Sidebar when sidebarOpen is true', () => {
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
  });

  it('hides Sidebar when sidebarOpen is false', () => {
    mockSidebarOpen.mockReturnValue(false);
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.queryByTestId('sidebar')).not.toBeInTheDocument();
  });

  it('adds sidebar-collapsed class when sidebar is closed', () => {
    mockSidebarOpen.mockReturnValue(false);
    const { container } = render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(container.firstChild).toHaveClass('app-shell--sidebar-collapsed');
  });

  it('toggles sidebar when overlay is clicked on mobile', () => {
    mockIsMobile.mockReturnValue(true);
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    const overlay = screen.getByTestId('sidebar-overlay');
    expect(overlay).toBeInTheDocument();
    fireEvent.click(overlay);
    expect(mockToggleSidebar).toHaveBeenCalled();
  });

  // Added: Overlay is not rendered on desktop (TMAIL-33)
  it('does not show overlay on desktop when sidebar is open', () => {
    mockIsMobile.mockReturnValue(false);
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.queryByTestId('sidebar-overlay')).not.toBeInTheDocument();
  });

  // Added: Desktop always shows sidebar even when sidebarOpen is false (TMAIL-33)
  it('shows sidebar on desktop even when sidebarOpen is false', () => {
    mockIsMobile.mockReturnValue(false);
    mockSidebarOpen.mockReturnValue(false);
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
  });

  // Added: View mode routing tests.
  // TMAIL-399: managers that moved to SettingsHub (Two-Factor, Push Devices,
  // Signatures, Vacation, Filters, Spam, SMTP, POP3, DAV, Migration, Shared
  // Files, Groups, AI Config, Ollama, Low Bandwidth) are NOT in this list —
  // they're covered by SettingsHub.test.tsx now.
  const viewModeTests = [
    { viewMode: 'list', testId: 'message-list' },
    { viewMode: 'reader', testId: 'message-view' },
    { viewMode: 'compose', testId: 'composer' },
    { viewMode: 'search', testId: 'search-results' },
    { viewMode: 'contacts', testId: 'contact-manager' },
  ];

  // Changed (TMAIL-259): every non-list/reader view is now React.lazy() so
  // its module loads on first hit. findByTestId waits for Suspense to swap
  // the fallback for the resolved mock; the eager list/reader cases still
  // resolve synchronously through the original getByTestId path.
  it.each(viewModeTests)(
    'renders $testId when viewMode is "$viewMode"',
    async ({ viewMode, testId }) => {
      mockViewMode.mockReturnValue(viewMode);
      render(<AppShell onLogout={mockLogout} />, { wrapper });
      expect(await screen.findByTestId(testId)).toBeInTheDocument();
    },
  );

  it('does not render other view components when viewMode is list', () => {
    mockViewMode.mockReturnValue('list');
    render(<AppShell onLogout={mockLogout} />, { wrapper });
    expect(screen.getByTestId('message-list')).toBeInTheDocument();
    expect(screen.queryByTestId('message-view')).not.toBeInTheDocument();
    expect(screen.queryByTestId('composer')).not.toBeInTheDocument();
    expect(screen.queryByTestId('search-results')).not.toBeInTheDocument();
  });

  // TMAIL-399: the content prop overrides the viewMode ladder. /app/settings/*
  // uses this to mount SettingsHub inside the same chrome.
  it('renders content prop instead of viewMode content when provided', () => {
    mockViewMode.mockReturnValue('list');
    render(
      <AppShell
        onLogout={mockLogout}
        content={<div data-testid="route-override">Override</div>}
      />,
      { wrapper },
    );
    expect(screen.getByTestId('route-override')).toBeInTheDocument();
    // The viewMode-driven MessageList must NOT render when content is provided.
    expect(screen.queryByTestId('message-list')).not.toBeInTheDocument();
  });
});
