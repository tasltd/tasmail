import { X } from 'lucide-react';
import { useSearch } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatMessageDate } from '../../utils/date';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import type { MessageEnvelope } from '../../types/mail';

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

export function SearchResults() {
  const searchQuery = useMailStore((s) => s.searchQuery);
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const { data, isLoading, error } = useSearch(searchQuery, selectedFolder);

  const clearSearch = () => setSearchQuery('');

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
