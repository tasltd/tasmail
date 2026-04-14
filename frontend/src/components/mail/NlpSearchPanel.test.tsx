// Added: Tests for NlpSearchPanel component (TMAIL-135)

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { NlpSearchPanel } from './NlpSearchPanel';

// Added: Mock the NLP search API
const mockNlpSearch = vi.fn();
const mockListNlpHistory = vi.fn();
const mockClearNlpHistory = vi.fn();

vi.mock('../../api/nlp-search', () => ({
  nlpSearch: (...args: unknown[]) => mockNlpSearch(...args),
  listNlpHistory: (...args: unknown[]) => mockListNlpHistory(...args),
  clearNlpHistory: (...args: unknown[]) => mockClearNlpHistory(...args),
}));

// Added: Mock the mail store
const mockSetSelectedUid = vi.fn();
const mockSetSelectedFolder = vi.fn();

vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({
      setSelectedUid: mockSetSelectedUid,
      setSelectedFolder: mockSetSelectedFolder,
    }),
}));

describe('NlpSearchPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders search input with AI placeholder', () => {
    render(<NlpSearchPanel />);
    expect(screen.getByTestId('nlp-search-input')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/ask ai/i)).toBeInTheDocument();
  });

  it('renders Ask AI button', () => {
    render(<NlpSearchPanel />);
    const searchButton = screen.getByTestId('nlp-search-btn');
    expect(searchButton).toBeInTheDocument();
    expect(searchButton).toHaveTextContent('Ask AI');
  });

  it('renders history button', () => {
    render(<NlpSearchPanel />);
    const historyButton = screen.getByTestId('nlp-search-history-btn');
    expect(historyButton).toBeInTheDocument();
    expect(historyButton).toHaveTextContent('History');
  });

  it('shows parsed parameters after successful search', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'emails from John about budget',
      parsed_params: {
        from: 'John',
        subject: 'budget',
        keywords: ['budget'],
      },
      result_count: 0,
      results: [],
    });

    render(<NlpSearchPanel />);

    fireEvent.change(screen.getByTestId('nlp-search-input'), {
      target: { value: 'emails from John about budget' },
    });
    fireEvent.click(screen.getByTestId('nlp-search-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('nlp-search-parsed')).toBeInTheDocument();
    });

    expect(screen.getByTestId('nlp-search-parsed-params')).toHaveTextContent('From: John');
    expect(screen.getByTestId('nlp-search-parsed-params')).toHaveTextContent('Subject: budget');
  });

  it('shows empty results message when no matches found', async () => {
    mockNlpSearch.mockResolvedValue({
      query: 'nonexistent topic',
      parsed_params: { keywords: ['nonexistent'] },
      result_count: 0,
      results: [],
    });

    render(<NlpSearchPanel />);

    fireEvent.change(screen.getByTestId('nlp-search-input'), {
      target: { value: 'nonexistent topic search' },
    });
    fireEvent.click(screen.getByTestId('nlp-search-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('nlp-search-empty')).toBeInTheDocument();
    });
    expect(screen.getByText(/no matching emails found/i)).toBeInTheDocument();
  });

  it('shows search history when history button is clicked', async () => {
    mockListNlpHistory.mockResolvedValue([
      {
        id: 'hist-1',
        user_id: 'user-1',
        query_text: 'emails about budget',
        parsed_params: { subject: 'budget', keywords: [] },
        result_count: 5,
        created_at: '2026-04-14T10:00:00Z',
      },
    ]);

    render(<NlpSearchPanel />);

    fireEvent.click(screen.getByTestId('nlp-search-history-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('nlp-search-history')).toBeInTheDocument();
    });

    const historyItems = screen.getAllByTestId('nlp-search-history-item');
    expect(historyItems).toHaveLength(1);
    expect(historyItems[0]).toHaveTextContent('emails about budget');
  });

  it('shows error message on search failure', async () => {
    mockNlpSearch.mockRejectedValue(new Error('No active AI configuration found'));

    render(<NlpSearchPanel />);

    fireEvent.change(screen.getByTestId('nlp-search-input'), {
      target: { value: 'test query for failure' },
    });
    fireEvent.click(screen.getByTestId('nlp-search-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('nlp-search-error')).toBeInTheDocument();
    });
    expect(screen.getByText(/no active ai configuration found/i)).toBeInTheDocument();
  });

  it('disables search button when query is too short', () => {
    render(<NlpSearchPanel />);

    fireEvent.change(screen.getByTestId('nlp-search-input'), {
      target: { value: 'ab' },
    });

    expect(screen.getByTestId('nlp-search-btn')).toBeDisabled();
  });
});
