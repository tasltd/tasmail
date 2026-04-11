import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
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
vi.mock('../settings/SignatureManager', () => ({
  SignatureManager: () => <div data-testid="signature-manager">SignatureManager</div>,
}));
vi.mock('../settings/ContactManager', () => ({
  ContactManager: () => <div data-testid="contact-manager">ContactManager</div>,
}));
vi.mock('../settings/TwoFactorManager', () => ({
  TwoFactorManager: () => <div data-testid="two-factor-manager">TwoFactorManager</div>,
}));
vi.mock('../settings/VacationResponder', () => ({
  VacationResponder: () => <div data-testid="vacation-responder">VacationResponder</div>,
}));
vi.mock('../settings/GroupManager', () => ({
  GroupManager: () => <div data-testid="group-manager">GroupManager</div>,
}));
vi.mock('../settings/MigrationManager', () => ({
  MigrationManager: () => <div data-testid="migration-manager">MigrationManager</div>,
}));
vi.mock('../settings/LowBandwidthSettings', () => ({
  LowBandwidthSettings: () => <div data-testid="low-bandwidth">LowBandwidthSettings</div>,
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('AppShell', () => {
  const mockLogout = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockViewMode.mockReturnValue('list');
    mockSidebarOpen.mockReturnValue(true);
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

  it('toggles sidebar when overlay is clicked', () => {
    const { container } = render(<AppShell onLogout={mockLogout} />, { wrapper });
    const overlay = container.querySelector('.sidebar-overlay');
    expect(overlay).toBeInTheDocument();
    fireEvent.click(overlay!);
    expect(mockToggleSidebar).toHaveBeenCalled();
  });

  // Added: View mode routing tests
  const viewModeTests = [
    { viewMode: 'list', testId: 'message-list' },
    { viewMode: 'reader', testId: 'message-view' },
    { viewMode: 'compose', testId: 'composer' },
    { viewMode: 'search', testId: 'search-results' },
    { viewMode: 'signatures', testId: 'signature-manager' },
    { viewMode: 'contacts', testId: 'contact-manager' },
    { viewMode: 'security', testId: 'two-factor-manager' },
    { viewMode: 'vacation', testId: 'vacation-responder' },
    { viewMode: 'groups', testId: 'group-manager' },
    { viewMode: 'migration', testId: 'migration-manager' },
    { viewMode: 'bandwidth', testId: 'low-bandwidth' },
  ];

  it.each(viewModeTests)(
    'renders $testId when viewMode is "$viewMode"',
    ({ viewMode, testId }) => {
      mockViewMode.mockReturnValue(viewMode);
      render(<AppShell onLogout={mockLogout} />, { wrapper });
      expect(screen.getByTestId(testId)).toBeInTheDocument();
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
});
