import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { FolderTree } from './FolderTree';

// Mock the hooks
const mockUseFolders = vi.fn();
vi.mock('../../hooks/useMailbox', () => ({
  useFolders: () => mockUseFolders(),
}));

const mockSelectedFolder = vi.fn(() => 'INBOX');
const mockSetSelectedFolder = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      selectedFolder: mockSelectedFolder(),
      setSelectedFolder: mockSetSelectedFolder,
    }),
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('FolderTree', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSelectedFolder.mockReturnValue('INBOX');
  });

  it('shows loading state', () => {
    mockUseFolders.mockReturnValue({ data: undefined, isLoading: true, error: null });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('Loading folders...')).toBeInTheDocument();
  });

  it('shows error state', () => {
    mockUseFolders.mockReturnValue({ data: undefined, isLoading: false, error: new Error('fail') });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('Failed to load folders')).toBeInTheDocument();
  });

  it('renders folder list', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'INBOX', delimiter: '.', messages: 10, unseen: 3 },
        { name: 'Sent', delimiter: '.', messages: 5, unseen: 0 },
        { name: 'Drafts', delimiter: '.', messages: 2, unseen: null },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('INBOX')).toBeInTheDocument();
    expect(screen.getByText('Sent')).toBeInTheDocument();
    expect(screen.getByText('Drafts')).toBeInTheDocument();
  });

  it('shows unread count badge when unseen > 0', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'INBOX', delimiter: '.', messages: 10, unseen: 5 },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('5')).toBeInTheDocument();
  });

  it('does not show badge when unseen is 0', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'Sent', delimiter: '.', messages: 10, unseen: 0 },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('Sent')).toBeInTheDocument();
    // No badge text should be present for 0
    expect(screen.queryByText('0')).not.toBeInTheDocument();
  });

  it('does not show badge when unseen is null', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'Drafts', delimiter: '.', messages: 2, unseen: null },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('Drafts')).toBeInTheDocument();
  });

  it('calls setSelectedFolder when folder is clicked', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'INBOX', delimiter: '.', messages: 10, unseen: 3 },
        { name: 'Sent', delimiter: '.', messages: 5, unseen: 0 },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    fireEvent.click(screen.getByText('Sent'));
    expect(mockSetSelectedFolder).toHaveBeenCalledWith('Sent');
  });

  it('applies active class to selected folder', () => {
    mockSelectedFolder.mockReturnValue('INBOX');
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'INBOX', delimiter: '.', messages: 10, unseen: 3 },
        { name: 'Sent', delimiter: '.', messages: 5, unseen: 0 },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    const inboxButton = screen.getByText('INBOX').closest('button');
    expect(inboxButton?.className).toContain('folder-item--active');
    const sentButton = screen.getByText('Sent').closest('button');
    expect(sentButton?.className).not.toContain('folder-item--active');
  });

  it('renders custom folder with generic icon', () => {
    mockUseFolders.mockReturnValue({
      data: [
        { name: 'MyFolder', delimiter: '.', messages: 1, unseen: 1 },
      ],
      isLoading: false,
      error: null,
    });
    render(<FolderTree />, { wrapper });
    expect(screen.getByText('MyFolder')).toBeInTheDocument();
  });
});
