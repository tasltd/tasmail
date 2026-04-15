import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Sidebar } from './Sidebar';

// Mock the mail store
const mockSetViewMode = vi.fn();
const mockViewMode = vi.fn(() => 'inbox');
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      setViewMode: mockSetViewMode,
      viewMode: mockViewMode(),
    }),
}));

// Added: Mock uiStore for mobile sidebar close behavior (TMAIL-33)
const mockSetSidebarOpen = vi.fn();
vi.mock('../../stores/uiStore', () => ({
  useUiStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      setSidebarOpen: mockSetSidebarOpen,
    }),
}));

// Added: Mock useResponsive hook — defaults to desktop (TMAIL-33)
const mockIsMobile = vi.fn(() => false);
vi.mock('../../hooks/useResponsive', () => ({
  useResponsive: () => ({
    isMobile: mockIsMobile(),
    isTablet: false,
    isDesktop: true,
  }),
}));

// Mock child components as simple divs
vi.mock('../mail/FolderTree', () => ({
  FolderTree: () => <div data-testid="folder-tree">FolderTree</div>,
}));

vi.mock('./QuotaBar', () => ({
  QuotaBar: () => <div data-testid="quota-bar">QuotaBar</div>,
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('Sidebar', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockViewMode.mockReturnValue('inbox');
    mockIsMobile.mockReturnValue(false);
  });

  it('renders Compose button and calls setViewMode on click', () => {
    render(<Sidebar />, { wrapper });
    const composeBtn = screen.getByText('Compose');
    expect(composeBtn).toBeInTheDocument();
    fireEvent.click(composeBtn);
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
  });

  it('renders FolderTree component', () => {
    render(<Sidebar />, { wrapper });
    expect(screen.getByTestId('folder-tree')).toBeInTheDocument();
  });

  it('renders QuotaBar component', () => {
    render(<Sidebar />, { wrapper });
    expect(screen.getByTestId('quota-bar')).toBeInTheDocument();
  });

  const navItems = [
    { label: 'Signatures', viewMode: 'signatures' },
    { label: 'Contacts', viewMode: 'contacts' },
    { label: 'Security', viewMode: 'security' },
    { label: 'Vacation', viewMode: 'vacation' },
    { label: 'Groups', viewMode: 'groups' },
    { label: 'Migration', viewMode: 'migration' },
    { label: 'Bandwidth', viewMode: 'bandwidth' },
  ];

  it.each(navItems)('renders $label nav button', ({ label }) => {
    render(<Sidebar />, { wrapper });
    expect(screen.getByText(label)).toBeInTheDocument();
  });

  it.each(navItems)(
    'clicking $label calls setViewMode with "$viewMode"',
    ({ label, viewMode }) => {
      render(<Sidebar />, { wrapper });
      fireEvent.click(screen.getByText(label));
      expect(mockSetViewMode).toHaveBeenCalledWith(viewMode);
    },
  );

  it('applies active class to the button matching current viewMode', () => {
    mockViewMode.mockReturnValue('contacts');
    render(<Sidebar />, { wrapper });
    const contactsBtn = screen.getByText('Contacts').closest('button');
    expect(contactsBtn?.className).toContain('folder-item--active');
    // Other buttons should not have the active class
    const signaturesBtn = screen.getByText('Signatures').closest('button');
    expect(signaturesBtn?.className).not.toContain('folder-item--active');
  });

  it('no nav button has active class when viewMode is inbox', () => {
    mockViewMode.mockReturnValue('inbox');
    render(<Sidebar />, { wrapper });
    for (const { label } of navItems) {
      const btn = screen.getByText(label).closest('button');
      expect(btn?.className).not.toContain('folder-item--active');
    }
  });

  // Added: Mobile sidebar auto-close tests (TMAIL-33)
  it('closes sidebar on mobile after clicking a nav item', () => {
    mockIsMobile.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Contacts'));
    expect(mockSetViewMode).toHaveBeenCalledWith('contacts');
    expect(mockSetSidebarOpen).toHaveBeenCalledWith(false);
  });

  it('does not close sidebar on desktop after clicking a nav item', () => {
    mockIsMobile.mockReturnValue(false);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Contacts'));
    expect(mockSetViewMode).toHaveBeenCalledWith('contacts');
    expect(mockSetSidebarOpen).not.toHaveBeenCalled();
  });

  it('closes sidebar on mobile after clicking Compose', () => {
    mockIsMobile.mockReturnValue(true);
    render(<Sidebar />, { wrapper });
    fireEvent.click(screen.getByText('Compose'));
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
    expect(mockSetSidebarOpen).toHaveBeenCalledWith(false);
  });
});
