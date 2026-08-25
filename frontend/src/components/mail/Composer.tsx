import { useState, useEffect, useRef, useCallback } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Send, X, Save, Clock, Undo2, Sparkles, Paperclip, CalendarPlus } from 'lucide-react';
import { saveDraft } from '../../api/messages';
import { scheduledApi } from '../../api/scheduled';
import { useMailStore } from '../../stores/mailStore';
// Added: Import AiComposePanel for AI-powered draft generation (TMAIL-134)
import { AiComposePanel } from './AiComposePanel';
// Added: Large file auto-upload widget (TMAIL-138)
import { LargeFileAttacher } from './LargeFileAttacher';
// Added: Recipient autocomplete from contacts (TMAIL-119)
import { RecipientAutocomplete } from './RecipientAutocomplete';
// Added: Schedule Meeting modal launched from the composer toolbar (TMAIL-127)
import { ScheduleMeetingModal } from './ScheduleMeetingModal';
// Added (TMAIL-89): offline-first draft persistence + reconnect sync.
import {
  type OfflineDraft,
  type OfflineDraftAttachment,
  createEmptyDraft,
  applyEdits,
  saveDraftLocal,
  loadDraft,
  addAttachment,
  removeAttachment,
  statusBadge,
  syncOne,
  newLocalId,
  AttachmentQuotaError,
  isDirty as isDraftDirty,
  markError,
} from '../../utils/offline-drafts';
import { useOnlineStatus } from '../../hooks/useOnlineStatus';

export function Composer() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const isOnline = useOnlineStatus();
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [subject, setSubject] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [draftStatus, setDraftStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [undoState, setUndoState] = useState<{ cancelToken: string; countdown: number } | null>(null);
  const [showSchedulePicker, setShowSchedulePicker] = useState(false);
  const [scheduleDate, setScheduleDate] = useState('');
  // Added: AI compose panel toggle state (TMAIL-134)
  const [showAiCompose, setShowAiCompose] = useState(false);
  // Added: Large file attacher toggle state (TMAIL-138)
  const [showLargeFile, setShowLargeFile] = useState(false);
  // Added: Schedule Meeting modal toggle state (TMAIL-127)
  const [showMeetingModal, setShowMeetingModal] = useState(false);
  const draftTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const undoTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Added (TMAIL-89): offline draft state. `draftRef` is the source of truth
  // for what's currently in IndexedDB; React state holds form fields for the
  // editor. We keep a stable localId across the Composer's mount so reload =
  // restore.
  const draftRef = useRef<OfflineDraft>(createEmptyDraft());
  const [attachments, setAttachments] = useState<OfflineDraftAttachment[]>([]);
  const [draftSyncStatus, setDraftSyncStatus] = useState<OfflineDraft['status']>('local');
  const [draftAttachmentError, setDraftAttachmentError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Link.configure({ openOnClick: false }),
      Placeholder.configure({ placeholder: 'Write your email...' }),
    ],
    content: '',
  });

  // Added (TMAIL-89): build a snapshot of the current form state as an
  // OfflineDraft and persist it locally. Local writes are cheap and happen
  // immediately on every edit so a crash / reload never loses keystrokes.
  const persistLocalDraft = useCallback(async (): Promise<OfflineDraft> => {
    const snapshot = applyEdits(draftRef.current, {
      to,
      cc,
      subject,
      htmlBody: editor?.getHTML() || '',
      textBody: editor?.getText() || '',
    });
    draftRef.current = snapshot;
    setDraftSyncStatus(snapshot.status);
    await saveDraftLocal(snapshot);
    return snapshot;
  }, [to, cc, subject, editor]);

  // Added: Debounced server-side draft save. Writes locally first (always),
  // then — only if online and the draft has real content — pushes to
  // /api/drafts. When offline, the local copy is the only persistence; the
  // reconnect effect below replays it.
  const saveDraftNow = useCallback(async () => {
    // NOTE: Keep the original to/subject guard so a totally empty composer
    // doesn't generate phantom drafts. Editor content alone isn't a strong
    // enough signal — see the "does not auto-save when both to and subject
    // are empty" test.
    if (!to.trim() && !subject.trim()) return;
    setDraftStatus('saving');
    const snapshot = await persistLocalDraft();

    if (!navigator.onLine) {
      // Stay 'local' — reconnect effect will pick this up.
      setDraftStatus('saved');
      setDraftSyncStatus(snapshot.status);
      return;
    }

    const result = await syncOne(snapshot, {
      postDraft: async (d) => {
        await saveDraft({
          to: d.to.split(',').map((s) => s.trim()).filter(Boolean),
          cc: d.cc ? d.cc.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
          subject: d.subject || '(no subject)',
          html_body: d.htmlBody || undefined,
          text_body: d.textBody || undefined,
        });
        return { status: 'ok' };
      },
    });
    draftRef.current = result;
    setDraftSyncStatus(result.status);
    setDraftStatus(result.status === 'synced' ? 'saved' : 'idle');
    // `cc` and `editor` are read inside the early-return guard via to/subject —
    // persistLocalDraft already owns the rest. The exhaustive-deps lint wants
    // us to declare what's read at this scope.
  }, [to, subject, persistLocalDraft]);

  const scheduleDraftSave = useCallback(() => {
    if (draftTimerRef.current) clearTimeout(draftTimerRef.current);
    // Fire-and-forget the immediate local save so reload-protection is instant.
    void persistLocalDraft();
    draftTimerRef.current = setTimeout(() => {
      saveDraftNow();
    }, 5000);
  }, [saveDraftNow, persistLocalDraft]);

  // Added (TMAIL-89): on mount, look for a draftId in the URL and rehydrate.
  // Lets a reload restore exactly what the user was working on.
  useEffect(() => {
    const search = typeof window !== 'undefined' ? window.location.search : '';
    const draftId = new URLSearchParams(search).get('draftId');
    if (!draftId) {
      // Persist the freshly-created empty draft so its localId becomes
      // restorable via ?draftId= once the user types anything.
      draftRef.current = createEmptyDraft(newLocalId());
      return;
    }
    (async () => {
      const restored = await loadDraft(draftId);
      if (!restored) {
        draftRef.current = createEmptyDraft(draftId);
        return;
      }
      draftRef.current = restored;
      setTo(restored.to);
      setCc(restored.cc);
      setSubject(restored.subject);
      setAttachments(restored.attachments);
      setDraftSyncStatus(restored.status);
      if (editor && restored.htmlBody) {
        editor.commands.setContent(restored.htmlBody);
      }
    })();
    // NOTE: We deliberately do not list `editor` here — we run this once on
    // mount; editor content is set inside the effect when available.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Added (TMAIL-89): when connectivity returns, flush any dirty local draft
  // to the server so the badge eventually reads "Synced to server".
  // We only fire on the edge offline→online — not on initial mount — so a
  // freshly-opened Composer with an empty draft does NOT spam /api/drafts.
  const wasOfflineRef = useRef(false);
  useEffect(() => {
    const wasOffline = wasOfflineRef.current;
    wasOfflineRef.current = !isOnline;
    if (!isOnline || !wasOffline) return;
    if (!isDraftDirty(draftRef.current)) return;
    void saveDraftNow();
    // We only want to react to the online edge, not to every saveDraftNow change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOnline]);

  // Trigger auto-save on field changes
  useEffect(() => {
    scheduleDraftSave();
    return () => {
      if (draftTimerRef.current) clearTimeout(draftTimerRef.current);
    };
  }, [to, cc, subject, scheduleDraftSave]);

  // Also trigger on editor content changes
  useEffect(() => {
    if (!editor) return;
    const handler = () => scheduleDraftSave();
    editor.on('update', handler);
    return () => { editor.off('update', handler); };
  }, [editor, scheduleDraftSave]);

  // Added: Undo countdown cleanup
  useEffect(() => {
    return () => {
      if (undoTimerRef.current) clearInterval(undoTimerRef.current);
    };
  }, []);

  const handleSend = async () => {
    if (!to.trim()) {
      setError('Recipients required');
      return;
    }

    setSending(true);
    setError('');

    try {
      const htmlBody = editor?.getHTML() || '';
      const textBody = editor?.getText() || '';
      const recipients = to.split(',').map((s) => s.trim()).filter(Boolean);
      const ccList = cc ? cc.split(',').map((s) => s.trim()).filter(Boolean) : undefined;

      // Use schedule-send with 10s delay for undo capability
      const result = await scheduledApi.scheduleSend({
        to: recipients,
        cc: ccList,
        subject,
        html_body: htmlBody,
        text_body: textBody,
        delay_seconds: 10,
      });

      // Show undo toast with countdown
      setUndoState({ cancelToken: result.cancel_token, countdown: 10 });
      undoTimerRef.current = setInterval(() => {
        setUndoState((prev) => {
          if (!prev || prev.countdown <= 1) {
            if (undoTimerRef.current) clearInterval(undoTimerRef.current);
            return null;
          }
          return { ...prev, countdown: prev.countdown - 1 };
        });
      }, 1000);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send');
    } finally {
      setSending(false);
    }
  };

  const handleUndo = async () => {
    if (!undoState) return;
    try {
      await scheduledApi.cancelScheduled(undoState.cancelToken);
      setUndoState(null);
      if (undoTimerRef.current) clearInterval(undoTimerRef.current);
    } catch {
      setError('Failed to undo — message may have already been sent');
    }
  };

  // Added: Handler for when user accepts an AI-generated draft (TMAIL-134)
  const handleUseDraft = useCallback((draftSubject: string, draftBody: string) => {
    setSubject(draftSubject);
    if (editor) {
      editor.commands.setContent(draftBody.replace(/\n/g, '<br>'));
    }
    setShowAiCompose(false);
  }, [editor]);

  // Added: Handler for when LargeFileAttacher uploads a file successfully —
  // appends the generated download-link HTML to the editor body in place of
  // an inline attachment (TMAIL-138).
  const handleLargeFileLink = useCallback(
    (html: string) => {
      if (!editor) return;
      editor.commands.focus('end');
      editor.commands.insertContent(html);
    },
    [editor],
  );

  const handleLargeFileError = useCallback((msg: string) => {
    setError(msg);
  }, []);

  // Added (TMAIL-89): attachment picker — queues files as Blobs in the offline
  // attachment store. The reconnect sync flow will upload them once the
  // server-side draft endpoint accepts attachments (currently the backend
  // stores the meta on the IMAP draft; uploaded bytes ride along when the
  // server adds /api/drafts attachment support).
  const handleAttachFiles = useCallback(async (files: FileList | null) => {
    if (!files || files.length === 0) return;
    setDraftAttachmentError('');
    let next = await persistLocalDraft();
    for (const file of Array.from(files)) {
      try {
        const result = await addAttachment(next, {
          name: file.name,
          type: file.type,
          size: file.size,
          blob: file,
        });
        next = result.draft;
      } catch (err) {
        if (err instanceof AttachmentQuotaError) {
          setDraftAttachmentError(err.message);
        } else {
          next = markError(next, err instanceof Error ? err.message : 'Attachment failed');
        }
      }
    }
    draftRef.current = next;
    setAttachments(next.attachments);
    setDraftSyncStatus(next.status);
    await saveDraftLocal(next);
    if (fileInputRef.current) fileInputRef.current.value = '';
  }, [persistLocalDraft]);

  const handleRemoveAttachment = useCallback(async (attachmentId: string) => {
    const next = await removeAttachment(draftRef.current, attachmentId);
    draftRef.current = next;
    setAttachments(next.attachments);
    setDraftSyncStatus(next.status);
    await saveDraftLocal(next);
  }, []);

  const handleScheduleSend = async () => {
    if (!to.trim() || !scheduleDate) {
      setError('Recipients and schedule date required');
      return;
    }

    setSending(true);
    setError('');

    try {
      const htmlBody = editor?.getHTML() || '';
      const textBody = editor?.getText() || '';

      await scheduledApi.scheduleSend({
        to: to.split(',').map((s) => s.trim()).filter(Boolean),
        cc: cc ? cc.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
        subject,
        html_body: htmlBody,
        text_body: textBody,
        scheduled_at: new Date(scheduleDate).toISOString(),
      });

      setViewMode('list');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to schedule');
    } finally {
      setSending(false);
      setShowSchedulePicker(false);
    }
  };

  return (
    <div className="composer">
      <div className="composer__toolbar">
        <h3 data-testid="compose-header">New Message</h3>
        {/* Added (TMAIL-260): single live region so screen readers announce draft state transitions */}
        <span role="status" aria-live="polite" style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
          {draftStatus === 'saving' && 'Saving draft...'}
          {draftStatus === 'saved' && 'Draft saved'}
        </span>
        {/* Added (TMAIL-89): offline-vs-synced sync-status pill. Lives next to
            the existing saving/saved indicator so screen reader users hear both
            the in-flight transition AND the persistent status. */}
        <span
          data-testid="draft-sync-status"
          aria-live="polite"
          aria-label={`Draft status: ${statusBadge(draftSyncStatus).label}`}
          title={statusBadge(draftSyncStatus).label}
          style={{
            fontSize: '12px',
            marginLeft: '8px',
            padding: '2px 8px',
            borderRadius: '999px',
            background:
              statusBadge(draftSyncStatus).tone === 'good' ? 'var(--color-success-bg, #e6f4ea)' :
              statusBadge(draftSyncStatus).tone === 'warn' ? 'var(--color-warn-bg, #fdf3d8)' :
              statusBadge(draftSyncStatus).tone === 'error' ? 'var(--color-error-bg, #fde8e8)' :
              'var(--color-bg-subtle, #f0f0f0)',
            color:
              statusBadge(draftSyncStatus).tone === 'good' ? 'var(--color-success-fg, #1e7a36)' :
              statusBadge(draftSyncStatus).tone === 'warn' ? 'var(--color-warn-fg, #8a6d1c)' :
              statusBadge(draftSyncStatus).tone === 'error' ? 'var(--color-error-fg, #9b2c2c)' :
              'var(--color-text-secondary)',
          }}
        >
          {statusBadge(draftSyncStatus).label}
        </span>
        {/* Added (TMAIL-260): aria-label so SR users hear the action; title kept for mouse hover */}
        <button className="btn btn--icon" onClick={saveDraftNow} title="Save draft now" aria-label="Save draft now">
          <Save size={18} />
        </button>
        <button
          className="btn btn--icon"
          onClick={() => setViewMode('list')}
          title="Close composer"
          aria-label="Close composer"
        >
          <X size={20} />
        </button>
      </div>

      {/* Added (TMAIL-260): role=alert so screen readers announce send failures immediately */}
      {error && <div className="composer__error" role="alert">{error}</div>}

      <div className="composer__fields">
        <div className="composer__field">
          <label htmlFor="composer-to">To:</label>
          <RecipientAutocomplete
            inputId="composer-to"
            value={to}
            onChange={setTo}
            placeholder="recipient@example.com"
          />
        </div>
        <div className="composer__field">
          <label htmlFor="composer-cc">Cc:</label>
          <RecipientAutocomplete
            inputId="composer-cc"
            value={cc}
            onChange={setCc}
            placeholder="cc@example.com"
          />
        </div>
        <div className="composer__field">
          {/* Added (TMAIL-260): htmlFor / id wiring so clicking "Subject" focuses the input */}
          <label htmlFor="composer-subject">Subject:</label>
          <input
            id="composer-subject"
            type="text"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            placeholder="Subject"
          />
        </div>
      </div>

      {/* Added (TMAIL-89): offline attachment picker + list. Files go straight
          into IndexedDB so they survive reloads; the existing LargeFileAttacher
          remains the path for big-file uploads that should bypass IMAP draft
          size limits. */}
      <div className="composer__attachments" style={{ padding: '8px 16px', borderTop: '1px solid var(--color-border)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <input
            ref={fileInputRef}
            type="file"
            multiple
            data-testid="composer-attachment-input"
            style={{ display: 'none' }}
            onChange={(e) => handleAttachFiles(e.target.files)}
          />
          <button
            className="btn btn--secondary btn--sm"
            data-testid="composer-attach-btn"
            onClick={() => fileInputRef.current?.click()}
            type="button"
          >
            <Paperclip size={14} />
            Attach files
          </button>
          {attachments.length > 0 && (
            <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>
              {attachments.length} attachment{attachments.length === 1 ? '' : 's'} queued
            </span>
          )}
        </div>
        {draftAttachmentError && (
          <div role="alert" style={{ fontSize: '12px', color: 'var(--color-error-fg, #9b2c2c)', marginTop: '4px' }}>
            {draftAttachmentError}
          </div>
        )}
        {attachments.length > 0 && (
          <ul data-testid="composer-attachment-list" style={{ listStyle: 'none', padding: 0, margin: '8px 0 0', display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
            {attachments.map((a) => (
              <li
                key={a.id}
                style={{
                  display: 'inline-flex', alignItems: 'center', gap: '6px',
                  background: 'var(--color-bg-subtle, #f4f4f4)', borderRadius: '12px', padding: '4px 10px', fontSize: '12px',
                }}
              >
                <span title={a.filename}>{a.filename}</span>
                <span style={{ color: 'var(--color-text-secondary)' }}>({(a.size / 1024).toFixed(1)} KB)</span>
                <button
                  type="button"
                  aria-label={`Remove attachment ${a.filename}`}
                  onClick={() => handleRemoveAttachment(a.id)}
                  style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 0, color: 'inherit' }}
                >
                  <X size={12} />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="composer__editor">
        <EditorContent editor={editor} />
      </div>

      <div className="composer__actions">
        <button
          className="btn btn--primary"
          onClick={handleSend}
          disabled={sending}
        >
          <Send size={16} />
          {sending ? 'Sending...' : 'Send'}
        </button>
        <button
          className="btn btn--secondary"
          onClick={() => setShowSchedulePicker(!showSchedulePicker)}
          disabled={sending}
        >
          <Clock size={16} />
          Schedule
        </button>
        {/* Added: AI Compose toggle button for TMAIL-134 */}
        <button
          className="btn btn--secondary"
          data-testid="ai-compose-toggle"
          onClick={() => setShowAiCompose(!showAiCompose)}
          disabled={sending}
        >
          <Sparkles size={16} />
          AI Compose
        </button>
        {/* Added: Large file attacher toggle button for TMAIL-138 */}
        <button
          className="btn btn--secondary"
          data-testid="large-file-toggle"
          onClick={() => setShowLargeFile(!showLargeFile)}
          disabled={sending}
        >
          <Paperclip size={16} />
          Attach large file
        </button>
        {/* Added: Schedule Meeting button — opens modal pre-filled with To/Cc + subject (TMAIL-127) */}
        <button
          className="btn btn--secondary"
          data-testid="schedule-meeting-toggle"
          onClick={() => setShowMeetingModal(true)}
          disabled={sending}
          title="Schedule meeting with these recipients"
        >
          <CalendarPlus size={16} />
          Schedule meeting
        </button>
      </div>

      {/* Added: AI Compose panel that appears when toggled (TMAIL-134) */}
      {showAiCompose && (
        <div style={{ padding: '12px 16px', borderTop: '1px solid var(--color-border)' }}>
          <AiComposePanel onUseDraft={handleUseDraft} />
        </div>
      )}

      {/* Added: Large file attacher panel that appears when toggled (TMAIL-138) */}
      {showLargeFile && (
        <div style={{ padding: '12px 16px', borderTop: '1px solid var(--color-border)' }}>
          <LargeFileAttacher
            onLinkReady={handleLargeFileLink}
            onError={handleLargeFileError}
          />
        </div>
      )}

      {showSchedulePicker && (
        <div className="composer__schedule" style={{ padding: '8px 16px', borderTop: '1px solid var(--color-border)', display: 'flex', alignItems: 'center', gap: '8px' }}>
          {/* Added (TMAIL-260): htmlFor / id wiring for the Send-at picker */}
          <label htmlFor="composer-schedule-at" style={{ fontSize: '13px' }}>Send at:</label>
          <input
            id="composer-schedule-at"
            type="datetime-local"
            value={scheduleDate}
            onChange={(e) => setScheduleDate(e.target.value)}
            min={new Date().toISOString().slice(0, 16)}
            style={{ fontSize: '13px', padding: '4px 8px' }}
          />
          <button className="btn btn--primary btn--sm" onClick={handleScheduleSend} disabled={sending || !scheduleDate}>
            Schedule Send
          </button>
        </div>
      )}

      {/* Added: Schedule Meeting modal pre-populated from current To/Cc + subject (TMAIL-127) */}
      {showMeetingModal && (
        <ScheduleMeetingModal
          initialTitle={subject}
          initialAttendees={[
            ...to.split(',').map((s) => s.trim()).filter(Boolean),
            ...cc.split(',').map((s) => s.trim()).filter(Boolean),
          ]}
          onClose={() => setShowMeetingModal(false)}
        />
      )}

      {undoState && (
        // Added (TMAIL-260): role=status + aria-live=polite so screen readers announce
        // the "Message sent (Ns)" countdown and undo affordance without interrupting.
        <div
          className="composer__undo-toast"
          role="status"
          aria-live="polite"
          style={{
            position: 'fixed', bottom: '24px', left: '50%', transform: 'translateX(-50%)',
            background: 'var(--color-bg-elevated, #333)', color: 'var(--color-text-inverse, #fff)',
            padding: '12px 20px', borderRadius: '8px', display: 'flex', alignItems: 'center', gap: '12px',
            boxShadow: '0 4px 12px rgba(0,0,0,0.3)', zIndex: 1000,
          }}
        >
          <span>Message sent ({undoState.countdown}s)</span>
          <button onClick={handleUndo} style={{
            background: 'transparent', color: 'var(--color-primary, #4a90d9)',
            border: '1px solid var(--color-primary, #4a90d9)', borderRadius: '4px',
            padding: '4px 12px', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '4px',
          }}>
            <Undo2 size={14} />
            Undo
          </button>
        </div>
      )}
    </div>
  );
}
