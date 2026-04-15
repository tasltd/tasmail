// Added: NlpSearch component tests for TMAIL-135

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { NlpSearch } from './NlpSearch';

const mockNlpSearch = vi.fn();
const mockListNlpHistory = vi.fn();
const mockClearNlpHistory = vi.fn();

vi.mock('../../api/nlp-search', () => ({
  nlpSearch: (...args: unknown[]) => mockNlpSearch(...args),
  listNlpHistory: () => mockListNlpHistory(),
  clearNlpHistory: () => mockClearNlpHistory(),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('NlpSearch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders search input and search button', () => {
    render(<NlpSearch />, { wrapper: createWrapper() });

    expect(
      screen.getByPlaceholderText("Search with natural language, e.g. 'emails from John last week with attachments'"),
    ).toBeInTheDocument();
    expect(screen.getByText('Search')).toBeInTheDocument();
  });

  it('renders search history button', () => {
    render(<NlpSearch />, { wrapper: createWrapper() });
    expect(screen.getByTitle('Search History')).toBeInTheDocument();
  });

  it('shows search results after query submission', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'emails from Alice',
      parsed_params: {
        from: 'Alice',
        keywords: ['Alice'],
      },
      result_count: 2,
      results: [
        {
          folder: 'INBOX',
          uid: 10,
          subject: 'Meeting with Alice tomorrow',
          from: 'alice@example.com',
          date: '2026-04-14T10:00:00Z',
        },
        {
          folder: 'INBOX',
          uid: 11,
          subject: 'Alice sent the report',
          from: 'alice@example.com',
          date: '2026-04-13T09:00:00Z',
        },
      ],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.change(
      screen.getByPlaceholderText("Search with natural language, e.g. 'emails from John last week with attachments'"),
      { target: { value: 'emails from Alice' } },
    );
    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      // NOTE: Text contains special quote entities, use a matcher function
      expect(screen.getByText(/2 results? for/)).toBeInTheDocument();
    });

    expect(screen.getByTestId('search-result-0')).toBeInTheDocument();
    expect(screen.getByTestId('search-result-1')).toBeInTheDocument();
  });

  it('displays parsed parameters as badges', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'emails from Alice with attachments',
      parsed_params: {
        from: 'Alice',
        has_attachment: true,
        keywords: [],
      },
      result_count: 0,
      results: [],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.change(
      screen.getByPlaceholderText("Search with natural language, e.g. 'emails from John last week with attachments'"),
      { target: { value: 'emails from Alice with attachments' } },
    );
    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      expect(screen.getByTestId('parsed-params')).toBeInTheDocument();
    });

    // NOTE: Check for parsed param badges
    expect(screen.getByText('Alice')).toBeInTheDocument();
    expect(screen.getByText('Attachment')).toBeInTheDocument();
  });

  it('shows empty results message when no matches', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'nonexistent search',
      parsed_params: { keywords: [] },
      result_count: 0,
      results: [],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.change(
      screen.getByPlaceholderText("Search with natural language, e.g. 'emails from John last week with attachments'"),
      { target: { value: 'nonexistent search' } },
    );
    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      expect(screen.getByText('No emails matched your search. Try rephrasing your query.')).toBeInTheDocument();
    });
  });

  it('shows search history panel when history button is clicked', async () => {
    mockListNlpHistory.mockResolvedValue([
      {
        id: 'h1',
        user_id: 'u1',
        query_text: 'emails from Bob last month',
        parsed_params: { from: 'Bob', keywords: [] },
        result_count: 5,
        created_at: '2026-04-10T10:00:00Z',
      },
    ]);

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTitle('Search History'));

    await waitFor(() => {
      expect(screen.getByText('Search History')).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByTestId('history-h1')).toBeInTheDocument();
    });
  });

  it('shows empty history state', async () => {
    mockListNlpHistory.mockResolvedValue([]);

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTitle('Search History'));

    await waitFor(() => {
      expect(screen.getByText('No search history yet.')).toBeInTheDocument();
    });
  });

  it('calls clearNlpHistory when Clear All button is clicked', async () => {
    mockListNlpHistory.mockResolvedValue([
      {
        id: 'h1',
        user_id: 'u1',
        query_text: 'test query',
        parsed_params: { keywords: [] },
        result_count: 3,
        created_at: '2026-04-10T10:00:00Z',
      },
    ]);
    mockClearNlpHistory.mockResolvedValue({ deleted: 1, message: 'Cleared' });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTitle('Search History'));

    await waitFor(() => {
      expect(screen.getByText('Clear All')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Clear All'));

    await waitFor(() => {
      expect(mockClearNlpHistory).toHaveBeenCalled();
    });
  });

  it('highlights matching keywords in search results', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'budget report',
      parsed_params: {
        keywords: ['budget'],
      },
      result_count: 1,
      results: [
        {
          folder: 'INBOX',
          uid: 20,
          subject: 'Q4 budget review meeting',
          from: 'finance@example.com',
          date: '2026-04-14T10:00:00Z',
        },
      ],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.change(
      screen.getByPlaceholderText("Search with natural language, e.g. 'emails from John last week with attachments'"),
      { target: { value: 'budget report' } },
    );
    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      // NOTE: The keyword "budget" should be wrapped in a <mark> element
      const marks = document.querySelectorAll('mark');
      expect(marks.length).toBeGreaterThan(0);
      expect(marks[0].textContent).toBe('budget');
    });
  });

  it('clears search results when clear button is clicked', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'test',
      parsed_params: { keywords: [] },
      result_count: 1,
      results: [{ folder: 'INBOX', uid: 1, subject: 'Test', from: 'a@b.com', date: '2026-04-14T10:00:00Z' }],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    const searchInput = screen.getByPlaceholderText(
      "Search with natural language, e.g. 'emails from John last week with attachments'",
    );
    fireEvent.change(searchInput, { target: { value: 'test' } });
    fireEvent.click(screen.getByText('Search'));

    await waitFor(() => {
      expect(screen.getByTestId('search-result-0')).toBeInTheDocument();
    });

    // Added: Click the clear (X) button
    fireEvent.click(screen.getByTitle('Clear'));

    // NOTE: After clearing, results and input should be gone
    expect(screen.queryByTestId('search-result-0')).not.toBeInTheDocument();
  });

  it('re-runs search when history entry is clicked', async () => {
    mockListNlpHistory.mockResolvedValue([
      {
        id: 'h1',
        user_id: 'u1',
        query_text: 'invoices from March',
        parsed_params: { keywords: ['invoices'] },
        result_count: 3,
        created_at: '2026-04-10T10:00:00Z',
      },
    ]);
    mockNlpSearch.mockResolvedValue({
      query: 'invoices from March',
      parsed_params: { keywords: ['invoices'] },
      result_count: 3,
      results: [
        { folder: 'INBOX', uid: 30, subject: 'March invoices', from: 'billing@co.com', date: '2026-03-15T10:00:00Z' },
      ],
    });

    render(<NlpSearch />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTitle('Search History'));

    await waitFor(() => {
      expect(screen.getByTestId('history-h1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('history-h1'));

    await waitFor(() => {
      expect(mockNlpSearch).toHaveBeenCalled();
      expect(mockNlpSearch.mock.calls[0][0]).toBe('invoices from March');
    });
  });
});
