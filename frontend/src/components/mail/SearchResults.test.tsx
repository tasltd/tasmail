import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SearchResults } from './SearchResults';

const mockUseSearch = vi.fn();
const mockUseAdvancedSearch = vi.fn();
vi.mock('../../hooks/useMailbox', () => ({
  useSearch: (...args: unknown[]) => mockUseSearch(...args),
  // Added: Mock for advanced search hook (TMAIL-32)
  useAdvancedSearch: (...args: unknown[]) => mockUseAdvancedSearch(...args),
}));

const mockSetSearchQuery = vi.fn();
const mockSetSelectedUid = vi.fn();
// Changed: Added advancedSearch and setAdvancedSearch for TMAIL-32
const mockSetAdvancedSearch = vi.fn();
// Changed (TMAIL-32): make store fields mutable per-test so we can simulate
// different query / advanced-search inputs without redefining the mock.
const storeState: { searchQuery: string; advancedSearch: unknown } = {
  searchQuery: 'test search',
  advancedSearch: null,
};
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      get searchQuery() { return storeState.searchQuery; },
      selectedFolder: 'INBOX',
      setSearchQuery: mockSetSearchQuery,
      setSelectedUid: mockSetSelectedUid,
      get advancedSearch() { return storeState.advancedSearch; },
      setAdvancedSearch: mockSetAdvancedSearch,
    }),
}));

vi.mock('../../utils/date', () => ({
  formatMessageDate: (d: string | null) => d ?? '',
}));

describe('SearchResults', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Added: Default return for advanced search hook (not active by default)
    mockUseAdvancedSearch.mockReturnValue({ data: undefined, isLoading: false, error: null });
    // Reset per-test store state
    storeState.searchQuery = 'test search';
    storeState.advancedSearch = null;
  });

  it('shows loading skeleton when isLoading', () => {
    mockUseSearch.mockReturnValue({ data: undefined, isLoading: true, error: null });
    const { container } = render(<SearchResults />);
    expect(container.querySelector('.loading-skeleton')).not.toBeNull();
  });

  it('shows result count and search query', () => {
    mockUseSearch.mockReturnValue({
      data: { total: 3, messages: [] },
      isLoading: false,
      error: null,
    });
    render(<SearchResults />);
    expect(screen.getByText('3 results for "test search"')).toBeInTheDocument();
  });

  it('shows "No messages match your search" for empty results', () => {
    mockUseSearch.mockReturnValue({
      data: { total: 0, messages: [] },
      isLoading: false,
      error: null,
    });
    render(<SearchResults />);
    expect(screen.getByText('No messages match your search')).toBeInTheDocument();
  });

  it('shows "Search failed" on error', () => {
    mockUseSearch.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('network error'),
    });
    render(<SearchResults />);
    expect(screen.getByText('Search failed')).toBeInTheDocument();
  });

  it('renders message rows with from/subject/date', () => {
    mockUseSearch.mockReturnValue({
      data: {
        total: 1,
        messages: [
          {
            uid: 42,
            from: 'alice@example.com',
            subject: 'Hello World',
            date: '2026-04-10',
            flags: [],
          },
        ],
      },
      isLoading: false,
      error: null,
    });
    render(<SearchResults />);
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('Hello World')).toBeInTheDocument();
    expect(screen.getByText('2026-04-10')).toBeInTheDocument();
  });

  // Added (TMAIL-32): keyword highlighting in result rows
  it('highlights query tokens that appear in from / subject', () => {
    storeState.searchQuery = 'budget alice';
    mockUseSearch.mockReturnValue({
      data: {
        total: 1,
        messages: [
          {
            uid: 1,
            from: 'alice@example.com',
            subject: 'Q3 budget review notes',
            date: '2026-04-10',
            flags: [],
          },
        ],
      },
      isLoading: false,
      error: null,
    });
    const { container } = render(<SearchResults />);
    const marks = Array.from(container.querySelectorAll('mark.search-highlight'));
    const matchedText = marks.map((m) => m.textContent?.toLowerCase());
    expect(matchedText).toContain('alice');
    expect(matchedText).toContain('budget');
  });

  // Added (TMAIL-32): advanced filter values also contribute to highlight set
  it('highlights tokens from advanced subject / from filters', () => {
    storeState.searchQuery = '';
    storeState.advancedSearch = { query: '', subject: 'invoice', from: 'bob' };
    mockUseAdvancedSearch.mockReturnValue({
      data: {
        total: 1,
        messages: [
          {
            uid: 2,
            from: 'bob@example.com',
            subject: 'March invoice attached',
            date: '2026-04-10',
            flags: [],
          },
        ],
      },
      isLoading: false,
      error: null,
    });
    const { container } = render(<SearchResults />);
    const matchedText = Array.from(container.querySelectorAll('mark.search-highlight')).map(
      (m) => m.textContent?.toLowerCase(),
    );
    expect(matchedText).toContain('invoice');
    expect(matchedText).toContain('bob');
  });

  it('clear search button calls setSearchQuery with empty string', () => {
    mockUseSearch.mockReturnValue({
      data: { total: 1, messages: [] },
      isLoading: false,
      error: null,
    });
    render(<SearchResults />);
    fireEvent.click(screen.getByTitle('Clear search'));
    expect(mockSetSearchQuery).toHaveBeenCalledWith('');
  });
});
