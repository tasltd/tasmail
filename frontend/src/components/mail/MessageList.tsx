import { useState, useMemo, useRef } from 'react';
import { Upload } from 'lucide-react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useCurrentMessages } from '../../hooks/useMailbox';
import { useMailStore } from '../../stores/mailStore';
// Added: Import drag hook for message drag-and-drop (TMAIL-122)
import { useMessageDrag } from '../../hooks/useDragAndDrop';
import { formatMessageDate } from '../../utils/date';
// Added: EML import for TMAIL-68
import { importEml } from '../../api/eml';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
// Added (TMAIL-401): empty-state copy with user's IMAP address when INBOX is empty.
import { EmptyInboxState } from './EmptyInboxState';
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
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  // Added: Drag handlers for message drag-and-drop (TMAIL-122)
  const { isDragging, ...dragHandlers } = useMessageDrag(message.uid, selectedFolder);

  const isRead = message.flags.some((f) => f.includes('Seen'));
  const isActive = selectedUid === message.uid;

  return (
    <div
      // Changed: Added drag handlers and dragging class for visual feedback (TMAIL-122)
      className={`message-row ${isActive ? 'message-row--active' : ''} ${!isRead ? 'message-row--unread' : ''} ${isDragging ? 'message-row--dragging' : ''}`}
      onClick={() => setSelectedUid(message.uid)}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && setSelectedUid(message.uid)}
      {...dragHandlers}
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
  const queryClient = useQueryClient();
  const { data, isLoading, error } = useCurrentMessages();
  const selectedFolder = useMailStore((s) => s.selectedFolder);
  const [threaded, setThreaded] = useState(true);
  // Added: Hidden file input ref for EML import (TMAIL-68)
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Added: EML import mutation — uploads .eml file to current folder (TMAIL-68)
  const importEmlMut = useMutation({
    mutationFn: (file: File) => importEml(selectedFolder, file),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      queryClient.invalidateQueries({ queryKey: ['folders'] });
    },
  });

  // Added: Handle file selection from the hidden input
  const handleEmlFileSelected = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = event.target.files?.[0];
    if (selectedFile) {
      importEmlMut.mutate(selectedFile);
    }
    // NOTE: Reset input value so the same file can be re-imported if needed
    if (fileInputRef.current) {
      fileInputRef.current.value = '';
    }
  };

  const threads = useMemo(() => {
    if (!data?.messages.length) return [];
    return groupByThread(data.messages);
  }, [data?.messages]);

  if (isLoading) return <LoadingSkeleton rows={10} />;
  // Changed (TMAIL-402): INBOX falls through to EmptyInboxState on error too —
  // a brand-new BYOK user whose IMAP isn't yet reachable should see the
  // welcoming "Messages sent to user@host will appear here" copy, not the
  // raw "Failed to load messages" string. Non-INBOX folders keep the
  // error message since the user is already deep in the app.
  if (error && selectedFolder !== 'INBOX') {
    return <div className="message-list__error">Failed to load messages</div>;
  }
  if (error || !data?.messages.length) {
    // TMAIL-401: INBOX gets the rich empty state with the user's configured
    // IMAP address; other folders keep the bare copy.
    if (selectedFolder === 'INBOX') {
      return <EmptyInboxState />;
    }
    return <div className="message-list__empty">No messages in this folder</div>;
  }

  return (
    <div className="message-list">
      {/* Added: Hidden file input for EML import (TMAIL-68) */}
      <input
        ref={fileInputRef}
        type="file"
        accept=".eml,message/rfc822"
        style={{ display: 'none' }}
        onChange={handleEmlFileSelected}
        data-testid="eml-import-input"
      />
      <div className="message-list__header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span>{data.total} messages</span>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          {/* Added: Import .eml button (TMAIL-68) */}
          <button
            className="btn btn--icon"
            onClick={() => fileInputRef.current?.click()}
            disabled={importEmlMut.isPending}
            title="Import .eml"
            data-testid="eml-import-button"
          >
            <Upload size={16} />
          </button>
          <label style={{ display: 'flex', gap: '4px', alignItems: 'center', fontSize: '12px', cursor: 'pointer' }}>
            <input type="checkbox" checked={threaded} onChange={(e) => setThreaded(e.target.checked)} />
            Conversations
          </label>
        </div>
      </div>
      <div className="message-list__items">
        {threaded
          ? threads.map((thread) => (
              // Fix (TMAIL-263): key by first message uid (stable) instead of array
              // index so new mail arriving doesn't force every ThreadRow to remount.
              <ThreadRow key={thread.messages[0]?.uid ?? thread.subject} thread={thread} />
            ))
          : data.messages.map((msg) => <MessageRow key={msg.uid} message={msg} />)
        }
      </div>
    </div>
  );
}
