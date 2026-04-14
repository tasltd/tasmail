import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { TopBar } from './TopBar';

// Mock stores and hooks
const mockToggleSidebar = vi.fn();
const mockToggleTheme = vi.fn();
const mockTheme = vi.fn(() => 'light');
vi.mock('../../stores/uiStore', () => ({
  useUiStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      toggleSidebar: mockToggleSidebar,
      theme: mockTheme(),
      toggleTheme: mockToggleTheme,
    }),
}));

const mockSetSearchQuery = vi.fn();
// Changed: Added advancedSearch fields required by AdvancedSearch child component
const mockSetAdvancedSearch = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      searchQuery: '',
      selectedFolder: 'INBOX',
      advancedSearch: null,
      setSearchQuery: mockSetSearchQuery,
      setAdvancedSearch: mockSetAdvancedSearch,
    }),
}));

const mockIsOnline = vi.fn(() => true);
vi.mock('../../hooks/useOnlineStatus', () => ({
  useOnlineStatus: () => mockIsOnline(),
}));

describe('TopBar', () => {
  const mockOnLogout = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockTheme.mockReturnValue('light');
    mockIsOnline.mockReturnValue(true);
  });

  it('calls toggleSidebar when menu button is clicked', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    // Menu button is the first btn--icon button
    const menuBtn = screen.getAllByRole('button')[0];
    fireEvent.click(menuBtn);
    expect(mockToggleSidebar).toHaveBeenCalledOnce();
  });

  it('submits search when input has >= 2 characters', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    const input = screen.getByPlaceholderText('Search emails...');
    fireEvent.change(input, { target: { value: 'test query' } });
    // Submit the form
    fireEvent.submit(input.closest('form')!);
    expect(mockSetSearchQuery).toHaveBeenCalledWith('test query');
  });

  it('does not submit search when input has < 2 characters', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    const input = screen.getByPlaceholderText('Search emails...');
    fireEvent.change(input, { target: { value: 'a' } });
    fireEvent.submit(input.closest('form')!);
    expect(mockSetSearchQuery).not.toHaveBeenCalled();
  });

  it('does not submit search when input is only whitespace', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    const input = screen.getByPlaceholderText('Search emails...');
    fireEvent.change(input, { target: { value: '   ' } });
    fireEvent.submit(input.closest('form')!);
    expect(mockSetSearchQuery).not.toHaveBeenCalled();
  });

  it('calls toggleTheme when theme button is clicked', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    const themeBtn = screen.getByTitle('Toggle theme');
    fireEvent.click(themeBtn);
    expect(mockToggleTheme).toHaveBeenCalledOnce();
  });

  it('calls onLogout when logout button is clicked', () => {
    render(<TopBar onLogout={mockOnLogout} />);
    const logoutBtn = screen.getByTitle('Logout');
    fireEvent.click(logoutBtn);
    expect(mockOnLogout).toHaveBeenCalledOnce();
  });

  it('shows Offline indicator when not online', () => {
    mockIsOnline.mockReturnValue(false);
    render(<TopBar onLogout={mockOnLogout} />);
    expect(screen.getByText('Offline')).toBeInTheDocument();
  });

  it('hides Offline indicator when online', () => {
    mockIsOnline.mockReturnValue(true);
    render(<TopBar onLogout={mockOnLogout} />);
    expect(screen.queryByText('Offline')).not.toBeInTheDocument();
  });
});
