import { X } from 'lucide-react';
import { useSearch, useAdvancedSearch } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatMessageDate } from '../../utils/date';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import type { MessageEnvelope } from '../../types/mail';
import type { AdvancedSearchParams } from '../../api/messages';

function SearchRow({ message }: { message: MessageEnvelope }) {
  const setSelectedUid = useMailStore((s) => s.setSelectedUid);

  const isRead = message.flags.some((f) => f.includes('Seen'));

  return (
    <div
      className={`message-row ${!isRead ? 'message-row--unread' : ''}`}
      onClick={() => setSelectedUid(message.uid)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && setSelectedUid(message.uid)}
    >
      <div className="message-row__from">{message.from || '(unknown)'}</div>
      <div className="message-row__subject">{message.subject || '(no subject)'}</div>
      <div className="message-row__date">{formatMessageDate(message.date)}</div>
    </div>
  );
}

// Added: Filter chip component for displaying active advanced filters
function FilterChip({ label, onRemove }: { label: string; onRemove: () => void }) {
  return (
    <span className="filter-chip" data-testid="filter-chip">
      {label}
      <button
        className="filter-chip__remove"
        onClick={onRemove}
        title={`Remove filter: ${label}`}
        data-testid={`remove-filter-${label.split(':')[0]?.trim().toLowerCase()}`}
      >
        <X size={12} />
      </button>
    </span>
  );
}

// Added: Build list of active filter labels from advanced search params
function getActiveFilterLabels(params: AdvancedSearchParams | null): { label: string; field: keyof AdvancedSearchParams }[] {
  if (!params) return [];
  const filterLabels: { label: string; field: keyof AdvancedSearchParams }[] = [];
  if (params.from) filterLabels.push({ label: `From: ${params.from}`, field: 'from' });
  if (params.to) filterLabels.push({ label: `To: ${params.to}`, field: 'to' });
  if (params.subject) filterLabels.push({ label: `Subject: ${params.subject}`, field: 'subject' });
  if (params.dateFrom) filterLabels.push({ label: `After: ${params.dateFrom}`, field: 'dateFrom' });
  if (params.dateTo) filterLabels.push({ label: `Before: ${params.dateTo}`, field: 'dateTo' });
  if (params.hasAttachment) filterLabels.push({ label: 'Has attachment', field: 'hasAttachment' });
  if (params.isUnread) filterLabels.push({ label: 'Unread only', field: 'isUnread' });
  if (params.isStarred) filterLabels.push({ label: 'Starred only', field: 'isStarred' });
  return filterLabels;
}

export function SearchResults() {
  const searchQuery = useMailStore((s) => s.searchQuery);
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const advancedSearch = useMailStore((s) => s.advancedSearch);
  const setAdvancedSearch = useMailStore((s) => s.setAdvancedSearch);

  // Changed: Use advanced search when advanced params are set, otherwise fall back to simple search
  const simpleResult = useSearch(searchQuery, selectedFolder);
  const advancedResult = useAdvancedSearch(advancedSearch);

  const isAdvanced = advancedSearch != null;
  const { data, isLoading, error } = isAdvanced ? advancedResult : simpleResult;

  const activeFilters = getActiveFilterLabels(advancedSearch);

  const clearSearch = () => {
    setSearchQuery('');
    setAdvancedSearch(null);
  };

  // Added: Remove a single advanced filter by field name
  const removeFilter = (field: keyof AdvancedSearchParams) => {
    if (!advancedSearch) return;
    const updatedParams = { ...advancedSearch };
    if (field === 'hasAttachment' || field === 'isUnread' || field === 'isStarred') {
      updatedParams[field] = false;
    } else {
      // NOTE: Use delete to remove optional string fields; 'query' is never in activeFilters
      delete updatedParams[field];
    }
    // NOTE: If no filters remain and query is empty, clear everything
    const hasRemainingFilters = getActiveFilterLabels(updatedParams).length > 0;
    if (!hasRemainingFilters && !updatedParams.query) {
      setAdvancedSearch(null);
      setSearchQuery('');
    } else {
      setAdvancedSearch(updatedParams);
    }
  };

  if (isLoading) return <LoadingSkeleton rows={8} />;

  return (
    <div className="message-list">
      <div className="message-list__header">
        <span>
          {data ? `${data.total} results for "${searchQuery}"` : `Searching for "${searchQuery}"...`}
        </span>
        <button className="btn btn--icon" onClick={clearSearch} title="Clear search">
          <X size={16} />
        </button>
      </div>

      {/* Added: Active filter chips displayed above results */}
      {activeFilters.length > 0 && (
        <div className="message-list__filters" data-testid="active-filters">
          {activeFilters.map(({ label, field }) => (
            <FilterChip key={field} label={label} onRemove={() => removeFilter(field)} />
          ))}
        </div>
      )}

      {error && <div className="message-list__error">Search failed</div>}
      {data && data.messages.length === 0 && (
        <div className="message-list__empty">No messages match your search</div>
      )}
      {data && (
        <div className="message-list__items">
          {data.messages.map((msg) => (
            <SearchRow key={msg.uid} message={msg} />
          ))}
        </div>
      )}
    </div>
  );
}
