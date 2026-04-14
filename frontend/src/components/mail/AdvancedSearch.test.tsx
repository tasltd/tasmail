/**
 * PURPOSE: Unit tests for AdvancedSearch filter panel (TMAIL-32)
 * CONSTRAINTS: Tests mock mailStore; does not test actual API calls
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { AdvancedSearch } from './AdvancedSearch';

const mockSetAdvancedSearch = vi.fn();
const mockSetSearchQuery = vi.fn();
let mockSearchQuery = 'test query';

vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      searchQuery: mockSearchQuery,
      selectedFolder: 'INBOX',
      advancedSearch: null,
      setAdvancedSearch: mockSetAdvancedSearch,
      setSearchQuery: mockSetSearchQuery,
    }),
}));

describe('AdvancedSearch', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSearchQuery = 'test query';
  });

  it('renders nothing when visible is false', () => {
    const { container } = render(<AdvancedSearch visible={false} />);
    expect(container.innerHTML).toBe('');
  });

  it('renders all filter fields when visible', () => {
    render(<AdvancedSearch visible={true} />);
    expect(screen.getByTestId('filter-from')).toBeInTheDocument();
    expect(screen.getByTestId('filter-to')).toBeInTheDocument();
    expect(screen.getByTestId('filter-subject')).toBeInTheDocument();
    expect(screen.getByTestId('filter-date-from')).toBeInTheDocument();
    expect(screen.getByTestId('filter-date-to')).toBeInTheDocument();
    expect(screen.getByTestId('filter-has-attachment')).toBeInTheDocument();
    expect(screen.getByTestId('filter-is-unread')).toBeInTheDocument();
    expect(screen.getByTestId('filter-is-starred')).toBeInTheDocument();
  });

  it('submitting with query sets advanced search params', () => {
    render(<AdvancedSearch visible={true} />);

    fireEvent.change(screen.getByTestId('filter-from'), { target: { value: 'alice@example.com' } });
    fireEvent.submit(screen.getByTestId('advanced-search-panel'));

    expect(mockSetAdvancedSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        query: 'test query',
        folder: 'INBOX',
        from: 'alice@example.com',
      }),
    );
  });

  it('clear button resets all fields and clears search', () => {
    render(<AdvancedSearch visible={true} />);

    // Added: Fill in some filters first
    fireEvent.change(screen.getByTestId('filter-from'), { target: { value: 'bob@test.com' } });
    fireEvent.change(screen.getByTestId('filter-subject'), { target: { value: 'Important' } });

    fireEvent.click(screen.getByTestId('advanced-search-clear'));

    expect(mockSetAdvancedSearch).toHaveBeenCalledWith(null);
    expect(mockSetSearchQuery).toHaveBeenCalledWith('');
    // NOTE: After clear, input values reset to empty via local state
    expect(screen.getByTestId('filter-from')).toHaveValue('');
    expect(screen.getByTestId('filter-subject')).toHaveValue('');
  });

  it('shows date validation error when dateFrom is after dateTo', () => {
    render(<AdvancedSearch visible={true} />);

    fireEvent.change(screen.getByTestId('filter-date-from'), { target: { value: '2026-05-01' } });
    fireEvent.change(screen.getByTestId('filter-date-to'), { target: { value: '2026-04-01' } });

    expect(screen.getByTestId('date-error')).toBeInTheDocument();
  });

  it('does not show date error when dateFrom is before dateTo', () => {
    render(<AdvancedSearch visible={true} />);

    fireEvent.change(screen.getByTestId('filter-date-from'), { target: { value: '2026-04-01' } });
    fireEvent.change(screen.getByTestId('filter-date-to'), { target: { value: '2026-05-01' } });

    expect(screen.queryByTestId('date-error')).toBeNull();
  });

  it('checkbox toggles update filter state', () => {
    render(<AdvancedSearch visible={true} />);

    const attachmentCheckbox = screen.getByTestId('filter-has-attachment') as HTMLInputElement;
    expect(attachmentCheckbox.checked).toBe(false);

    fireEvent.click(attachmentCheckbox);
    expect(attachmentCheckbox.checked).toBe(true);

    const unreadCheckbox = screen.getByTestId('filter-is-unread') as HTMLInputElement;
    fireEvent.click(unreadCheckbox);
    expect(unreadCheckbox.checked).toBe(true);

    const starredCheckbox = screen.getByTestId('filter-is-starred') as HTMLInputElement;
    fireEvent.click(starredCheckbox);
    expect(starredCheckbox.checked).toBe(true);
  });

  it('search button is disabled when no query and no filters', () => {
    // Changed: Override mock to return empty search query
    mockSearchQuery = '';

    render(<AdvancedSearch visible={true} />);

    expect(screen.getByTestId('advanced-search-submit')).toBeDisabled();
  });

  it('search button is enabled when a filter is set even without query', () => {
    mockSearchQuery = '';

    render(<AdvancedSearch visible={true} />);

    fireEvent.click(screen.getByTestId('filter-has-attachment'));

    expect(screen.getByTestId('advanced-search-submit')).not.toBeDisabled();
  });

  it('submitting includes checkbox filters in params', () => {
    render(<AdvancedSearch visible={true} />);

    fireEvent.click(screen.getByTestId('filter-has-attachment'));
    fireEvent.click(screen.getByTestId('filter-is-unread'));
    fireEvent.submit(screen.getByTestId('advanced-search-panel'));

    expect(mockSetAdvancedSearch).toHaveBeenCalledWith(
      expect.objectContaining({
        hasAttachment: true,
        isUnread: true,
      }),
    );
  });
});
