import { useCurrentMessages } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatMessageDate } from '../../utils/date';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import type { MessageEnvelope } from '../../types/mail';

function MessageRow({ message }: { message: MessageEnvelope }) {
  const selectedUid = useMailStore((s) => s.selectedUid);
  const setSelectedUid = useMailStore((s) => s.setSelectedUid);

  const isRead = message.flags.some((f) => f.includes('Seen'));
  const isActive = selectedUid === message.uid;

  return (
    <div
      className={`message-row ${isActive ? 'message-row--active' : ''} ${!isRead ? 'message-row--unread' : ''}`}
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

export function MessageList() {
  const { data, isLoading, error } = useCurrentMessages();

  if (isLoading) return <LoadingSkeleton rows={10} />;
  if (error) return <div className="message-list__error">Failed to load messages</div>;
  if (!data?.messages.length) {
    return <div className="message-list__empty">No messages in this folder</div>;
  }

  return (
    <div className="message-list">
      <div className="message-list__header">
        <span>{data.total} messages</span>
      </div>
      <div className="message-list__items">
        {data.messages.map((msg) => (
          <MessageRow key={msg.uid} message={msg} />
        ))}
      </div>
    </div>
  );
}
