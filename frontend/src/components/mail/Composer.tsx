import { useState, useEffect, useRef, useCallback } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Send, X, Save, Clock, Undo2 } from 'lucide-react';
import { saveDraft } from '../../api/messages';
import { scheduledApi } from '../../api/scheduled';
import { useMailStore } from '../../stores/mailStore';

export function Composer() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [subject, setSubject] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [draftStatus, setDraftStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [undoState, setUndoState] = useState<{ cancelToken: string; countdown: number } | null>(null);
  const [showSchedulePicker, setShowSchedulePicker] = useState(false);
  const [scheduleDate, setScheduleDate] = useState('');
  const draftTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const undoTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Link.configure({ openOnClick: false }),
      Placeholder.configure({ placeholder: 'Write your email...' }),
    ],
    content: '',
  });

  // Added: Debounced auto-save draft (5 second delay after last change)
  const saveDraftNow = useCallback(async () => {
    if (!to.trim() && !subject.trim()) return;
    setDraftStatus('saving');
    try {
      await saveDraft({
        to: to.split(',').map((s) => s.trim()).filter(Boolean),
        cc: cc ? cc.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
        subject: subject || '(no subject)',
        html_body: editor?.getHTML() || undefined,
        text_body: editor?.getText() || undefined,
      });
      setDraftStatus('saved');
    } catch {
      setDraftStatus('idle');
    }
  }, [to, cc, subject, editor]);

  const scheduleDraftSave = useCallback(() => {
    if (draftTimerRef.current) clearTimeout(draftTimerRef.current);
    draftTimerRef.current = setTimeout(() => {
      saveDraftNow();
    }, 5000);
  }, [saveDraftNow]);

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
        <h3>New Message</h3>
        {draftStatus === 'saving' && <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>Saving draft...</span>}
        {draftStatus === 'saved' && <span style={{ fontSize: '12px', color: 'var(--color-text-secondary)' }}>Draft saved</span>}
        <button className="btn btn--icon" onClick={saveDraftNow} title="Save draft now">
          <Save size={18} />
        </button>
        <button className="btn btn--icon" onClick={() => setViewMode('list')}>
          <X size={20} />
        </button>
      </div>

      {error && <div className="composer__error">{error}</div>}

      <div className="composer__fields">
        <div className="composer__field">
          <label>To:</label>
          <input
            type="text"
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="recipient@example.com"
          />
        </div>
        <div className="composer__field">
          <label>Cc:</label>
          <input
            type="text"
            value={cc}
            onChange={(e) => setCc(e.target.value)}
            placeholder="cc@example.com"
          />
        </div>
        <div className="composer__field">
          <label>Subject:</label>
          <input
            type="text"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            placeholder="Subject"
          />
        </div>
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
      </div>

      {showSchedulePicker && (
        <div className="composer__schedule" style={{ padding: '8px 16px', borderTop: '1px solid var(--color-border)', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <label style={{ fontSize: '13px' }}>Send at:</label>
          <input
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

      {undoState && (
        <div className="composer__undo-toast" style={{
          position: 'fixed', bottom: '24px', left: '50%', transform: 'translateX(-50%)',
          background: 'var(--color-bg-elevated, #333)', color: 'var(--color-text-inverse, #fff)',
          padding: '12px 20px', borderRadius: '8px', display: 'flex', alignItems: 'center', gap: '12px',
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)', zIndex: 1000,
        }}>
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
