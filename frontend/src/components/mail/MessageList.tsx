import { useState, useMemo } from 'react';
import { useCurrentMessages } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
import { formatMessageDate } from '../../utils/date';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
import type { MessageEnvelope } from '../../types/mail';

// Added: Normalize subject by stripping Re:/Fwd: prefixes for threading
function normalizeSubject(subject: string | null): string {
  if (!subject) return '';
  return subject.replace(/^(Re|Fwd|Fw):\s*/gi, '').trim().toLowerCase();
}

interface ThreadGroup {
  subject: string;
  messages: MessageEnvelope[];
  latestDate: string | null;
  hasUnread: boolean;
}

function groupByThread(messages: MessageEnvelope[]): ThreadGroup[] {
  const groups = new Map<string, ThreadGroup>();

  for (const msg of messages) {
    const key = normalizeSubject(msg.subject) || `uid-${msg.uid}`;
    const existing = groups.get(key);
    if (existing) {
      existing.messages.push(msg);
      if (!existing.hasUnread) {
        existing.hasUnread = !msg.flags.some((f) => f.includes('Seen'));
      }
    } else {
      groups.set(key, {
        subject: msg.subject || '(no subject)',
        messages: [msg],
        latestDate: msg.date,
        hasUnread: !msg.flags.some((f) => f.includes('Seen')),
      });
    }
  }

  return Array.from(groups.values());
}

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

// Added: Thread group row — shows thread count and expands on click
function ThreadRow({ thread }: { thread: ThreadGroup }) {
  const [expanded, setExpanded] = useState(false);
  const setSelectedUid = useMailStore((s) => s.setSelectedUid);

  if (thread.messages.length === 1) {
    return <MessageRow message={thread.messages[0]} />;
  }

  const latestMsg = thread.messages[0];

  return (
    <div>
      <div
        className={`message-row ${thread.hasUnread ? 'message-row--unread' : ''}`}
        onClick={() => setExpanded(!expanded)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => e.key === 'Enter' && setExpanded(!expanded)}
      >
        <div className="message-row__from">
          {latestMsg.from || '(unknown)'}
          <span style={{ marginLeft: '6px', fontSize: '11px', color: 'var(--color-text-secondary)', fontWeight: 400 }}>
            ({thread.messages.length})
          </span>
        </div>
        <div className="message-row__subject">{thread.subject}</div>
        <div className="message-row__date">{formatMessageDate(thread.latestDate)}</div>
      </div>
      {expanded && (
        <div style={{ paddingLeft: '16px', borderLeft: '2px solid var(--color-primary)' }}>
          {thread.messages.map((msg) => (
            <div
              key={msg.uid}
              className={`message-row ${!msg.flags.some((f) => f.includes('Seen')) ? 'message-row--unread' : ''}`}
              onClick={() => setSelectedUid(msg.uid)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => e.key === 'Enter' && setSelectedUid(msg.uid)}
              style={{ fontSize: '13px' }}
            >
              <div className="message-row__from">{msg.from || '(unknown)'}</div>
              <div className="message-row__subject" style={{ color: 'var(--color-text-secondary)' }}>
                {msg.subject || '(no subject)'}
              </div>
              <div className="message-row__date">{formatMessageDate(msg.date)}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function MessageList() {
  const { data, isLoading, error } = useCurrentMessages();
  const [threaded, setThreaded] = useState(true);

  const threads = useMemo(() => {
    if (!data?.messages.length) return [];
    return groupByThread(data.messages);
  }, [data?.messages]);

  if (isLoading) return <LoadingSkeleton rows={10} />;
  if (error) return <div className="message-list__error">Failed to load messages</div>;
  if (!data?.messages.length) {
    return <div className="message-list__empty">No messages in this folder</div>;
  }

  return (
    <div className="message-list">
      <div className="message-list__header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span>{data.total} messages</span>
        <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '12px', cursor: 'pointer' }}>
          <input type="checkbox" checked={threaded} onChange={(e) => setThreaded(e.target.checked)} />
          Conversations
        </label>
      </div>
      <div className="message-list__items">
        {threaded
          ? threads.map((thread, i) => <ThreadRow key={i} thread={thread} />)
          : data.messages.map((msg) => <MessageRow key={msg.uid} message={msg} />)
        }
      </div>
    </div>
  );
}
