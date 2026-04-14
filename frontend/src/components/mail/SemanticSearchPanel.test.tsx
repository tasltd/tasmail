// Added: Tests for SemanticSearchPanel component (TMAIL-106)

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SemanticSearchPanel } from './SemanticSearchPanel';

// Added: Mock the semantic search API
const mockSemanticSearch = vi.fn();
const mockGetIndexStats = vi.fn();

vi.mock('../../api/semantic-search', () => ({
  semanticSearch: (...args: unknown[]) => mockSemanticSearch(...args),
  getIndexStats: (...args: unknown[]) => mockGetIndexStats(...args),
  indexEmail: vi.fn(),
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

describe('SemanticSearchPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders search input', () => {
    render(<SemanticSearchPanel />);
    expect(screen.getByTestId('semantic-search-input')).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/search by meaning/i)).toBeInTheDocument();
  });

  it('renders search button', () => {
    render(<SemanticSearchPanel />);
    const searchButton = screen.getByTestId('semantic-search-btn');
    expect(searchButton).toBeInTheDocument();
    expect(searchButton).toHaveTextContent('Search');
  });

  it('renders results list after successful search', async () => {
    mockSemanticSearch.mockResolvedValue([
      {
        folder: 'INBOX',
        uid: 42,
        subject: 'Quarterly Report',
        similarity_score: 0.89,
      },
      {
        folder: 'Sent',
        uid: 15,
        subject: 'Budget Review',
        similarity_score: 0.72,
      },
    ]);

    render(<SemanticSearchPanel />);

    // Added: Type a query and submit
    fireEvent.change(screen.getByTestId('semantic-search-input'), {
      target: { value: 'quarterly budget' },
    });
    fireEvent.click(screen.getByTestId('semantic-search-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('semantic-search-results')).toBeInTheDocument();
    });

    const resultItems = screen.getAllByTestId('semantic-search-result-item');
    expect(resultItems).toHaveLength(2);
  });

  it('shows similarity scores as percentages', async () => {
    mockSemanticSearch.mockResolvedValue([
      {
        folder: 'INBOX',
        uid: 42,
        subject: 'Test Email',
        similarity_score: 0.85,
      },
    ]);

    render(<SemanticSearchPanel />);

    fireEvent.change(screen.getByTestId('semantic-search-input'), {
      target: { value: 'test query' },
    });
    fireEvent.click(screen.getByTestId('semantic-search-btn'));

    await waitFor(() => {
      const scoreElement = screen.getByTestId('semantic-search-score');
      expect(scoreElement).toHaveTextContent('85% match');
    });
  });

  it('shows index stats when stats button is clicked', async () => {
    mockGetIndexStats.mockResolvedValue({
      total_indexed: 150,
      per_folder: [
        { folder: 'INBOX', count: 100 },
        { folder: 'Sent', count: 50 },
      ],
    });

    render(<SemanticSearchPanel />);

    fireEvent.click(screen.getByTestId('semantic-search-stats-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('semantic-search-stats')).toBeInTheDocument();
    });
    expect(screen.getByText(/150/)).toBeInTheDocument();
  });

  it('shows empty results message when no matches found', async () => {
    mockSemanticSearch.mockResolvedValue([]);

    render(<SemanticSearchPanel />);

    fireEvent.change(screen.getByTestId('semantic-search-input'), {
      target: { value: 'nonexistent topic' },
    });
    fireEvent.click(screen.getByTestId('semantic-search-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('semantic-search-empty')).toBeInTheDocument();
    });
    expect(screen.getByText(/no similar emails found/i)).toBeInTheDocument();
  });
});
