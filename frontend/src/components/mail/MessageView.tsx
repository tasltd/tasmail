import { ArrowLeft, Reply, Forward, Trash2 } from 'lucide-react';
import { useCurrentMessage } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatFullDate } from '../../utils/date';
import { sanitizeHtml } from '../../utils/sanitize';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

export function MessageView() {
  const { data: message, isLoading, error } = useCurrentMessage();
  const setSelectedUid = useMailStore((s) => s.setSelectedUid);
  const setViewMode = useMailStore((s) => s.setViewMode);

  if (isLoading) return <LoadingSkeleton rows={8} />;
  if (error) return <div className="message-view__error">Failed to load message</div>;
  if (!message) return null;

  const handleReply = () => {
    setViewMode('compose');
  };

  // Sanitize HTML body using DOMPurify to prevent XSS before rendering
  const sanitizedBody = message.html_body ? sanitizeHtml(message.html_body) : null;

  return (
    <div className="message-view">
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setSelectedUid(null)} title="Back to list">
          <ArrowLeft size={20} />
        </button>
        <button className="btn btn--icon" onClick={handleReply} title="Reply">
          <Reply size={20} />
        </button>
        <button className="btn btn--icon" title="Forward">
          <Forward size={20} />
        </button>
        <button className="btn btn--icon btn--danger" title="Delete">
          <Trash2 size={20} />
        </button>
      </div>

      <div className="message-view__header">
        <h2 className="message-view__subject">{message.subject || '(no subject)'}</h2>
        <div className="message-view__meta">
          <div className="message-view__from">
            <strong>From:</strong> {message.from}
          </div>
          <div className="message-view__to">
            <strong>To:</strong> {message.to.join(', ')}
          </div>
          {message.cc.length > 0 && (
            <div className="message-view__cc">
              <strong>Cc:</strong> {message.cc.join(', ')}
            </div>
          )}
          <div className="message-view__date">
            {formatFullDate(message.date)}
          </div>
        </div>
      </div>

      {message.attachments.length > 0 && (
        <div className="message-view__attachments">
          <strong>Attachments:</strong>
          {message.attachments.map((att) => (
            <span key={att.part_id} className="attachment-chip">
              {att.filename} ({Math.round(att.size / 1024)}KB)
            </span>
          ))}
        </div>
      )}

      <div className="message-view__body">
        {sanitizedBody ? (
          <div
            className="message-view__html"
            dangerouslySetInnerHTML={{ __html: sanitizedBody }}
          />
        ) : (
          <pre className="message-view__text">{message.text_body}</pre>
        )}
      </div>
    </div>
  );
}
