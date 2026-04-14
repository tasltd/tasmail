/**
 * PURPOSE: Collapsible advanced search filter panel for TMAIL-32
 * CONSTRAINTS: Date-from must not be after date-to; at least one field required to search
 * EXTERNAL: Uses useMailStore for search state management
 */
import { useState, useCallback } from 'react';
import { Search, RotateCcw } from 'lucide-react';
import { useMailStore } from '../../stores/mailStore';
import type { AdvancedSearchParams } from '../../api/messages';

// Added: Props for controlling panel visibility externally
interface AdvancedSearchProps {
  visible: boolean;
}

// Added: Empty filter state factory for reset
function emptyFilters() {
  return {
    from: '',
    to: '',
    subject: '',
    dateFrom: '',
    dateTo: '',
    hasAttachment: false,
    isUnread: false,
    isStarred: false,
  };
}

export function AdvancedSearch({ visible }: AdvancedSearchProps) {
  const searchQuery = useMailStore((s) => s.searchQuery);
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const setAdvancedSearch = useMailStore((s) => s.setAdvancedSearch);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);

  const [filters, setFilters] = useState(emptyFilters());

  // Added: Date validation — dateFrom must not be after dateTo
  const hasDateError = filters.dateFrom && filters.dateTo && filters.dateFrom > filters.dateTo;

  // Added: Check if any filter field has a value
  const hasAnyFilter =
    !!filters.from ||
    !!filters.to ||
    !!filters.subject ||
    !!filters.dateFrom ||
    !!filters.dateTo ||
    filters.hasAttachment ||
    filters.isUnread ||
    filters.isStarred;

  // NOTE: Search requires either a query (>= 2 chars) or at least one advanced filter
  const canSearch = (searchQuery.length >= 2 || hasAnyFilter) && !hasDateError;

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      if (!canSearch) return;

      const params: AdvancedSearchParams = {
        query: searchQuery,
        folder: selectedFolder,
      };
      // Added: Only include non-empty filter values
      if (filters.from) params.from = filters.from;
      if (filters.to) params.to = filters.to;
      if (filters.subject) params.subject = filters.subject;
      if (filters.dateFrom) params.dateFrom = filters.dateFrom;
      if (filters.dateTo) params.dateTo = filters.dateTo;
      if (filters.hasAttachment) params.hasAttachment = true;
      if (filters.isUnread) params.isUnread = true;
      if (filters.isStarred) params.isStarred = true;

      setAdvancedSearch(params);
    },
    [canSearch, searchQuery, selectedFolder, filters, setAdvancedSearch],
  );

  const handleClear = useCallback(() => {
    setFilters(emptyFilters());
    setAdvancedSearch(null);
    setSearchQuery('');
  }, [setAdvancedSearch, setSearchQuery]);

  // Added: Update a single filter field
  const updateFilter = useCallback(
    (field: string, value: string | boolean) => {
      setFilters((prev) => ({ ...prev, [field]: value }));
    },
    [],
  );

  if (!visible) return null;

  return (
    <form className="advanced-search" onSubmit={handleSubmit} data-testid="advanced-search-panel">
      <div className="advanced-search__row">
        <label className="advanced-search__field">
          <span className="advanced-search__label">From</span>
          <input
            type="text"
            placeholder="sender@example.com"
            value={filters.from}
            onChange={(e) => updateFilter('from', e.target.value)}
            data-testid="filter-from"
          />
        </label>
        <label className="advanced-search__field">
          <span className="advanced-search__label">To</span>
          <input
            type="text"
            placeholder="recipient@example.com"
            value={filters.to}
            onChange={(e) => updateFilter('to', e.target.value)}
            data-testid="filter-to"
          />
        </label>
        <label className="advanced-search__field">
          <span className="advanced-search__label">Subject</span>
          <input
            type="text"
            placeholder="Email subject"
            value={filters.subject}
            onChange={(e) => updateFilter('subject', e.target.value)}
            data-testid="filter-subject"
          />
        </label>
      </div>

      <div className="advanced-search__row">
        <label className="advanced-search__field">
          <span className="advanced-search__label">From date</span>
          <input
            type="date"
            value={filters.dateFrom}
            onChange={(e) => updateFilter('dateFrom', e.target.value)}
            data-testid="filter-date-from"
          />
        </label>
        <label className="advanced-search__field">
          <span className="advanced-search__label">To date</span>
          <input
            type="date"
            value={filters.dateTo}
            onChange={(e) => updateFilter('dateTo', e.target.value)}
            data-testid="filter-date-to"
          />
        </label>
      </div>

      {/* Added: Date validation error message */}
      {hasDateError && (
        <div className="advanced-search__error" data-testid="date-error">
          &quot;From date&quot; cannot be after &quot;To date&quot;
        </div>
      )}

      <div className="advanced-search__row">
        <label className="advanced-search__checkbox">
          <input
            type="checkbox"
            checked={filters.hasAttachment}
            onChange={(e) => updateFilter('hasAttachment', e.target.checked)}
            data-testid="filter-has-attachment"
          />
          <span>Has attachment</span>
        </label>
        <label className="advanced-search__checkbox">
          <input
            type="checkbox"
            checked={filters.isUnread}
            onChange={(e) => updateFilter('isUnread', e.target.checked)}
            data-testid="filter-is-unread"
          />
          <span>Unread only</span>
        </label>
        <label className="advanced-search__checkbox">
          <input
            type="checkbox"
            checked={filters.isStarred}
            onChange={(e) => updateFilter('isStarred', e.target.checked)}
            data-testid="filter-is-starred"
          />
          <span>Starred only</span>
        </label>
      </div>

      <div className="advanced-search__actions">
        <button
          type="submit"
          className="btn btn--primary"
          disabled={!canSearch}
          data-testid="advanced-search-submit"
        >
          <Search size={16} /> Search
        </button>
        <button
          type="button"
          className="btn btn--secondary"
          onClick={handleClear}
          data-testid="advanced-search-clear"
        >
          <RotateCcw size={16} /> Clear
        </button>
      </div>
    </form>
  );
}
