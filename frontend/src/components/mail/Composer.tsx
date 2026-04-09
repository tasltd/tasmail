import { useState, useEffect, useRef, useCallback } from 'react';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Send, X, Save } from 'lucide-react';
import { sendMessage, saveDraft } from '../../api/messages';
import { useMailStore } from '../../stores/mailStore';

export function Composer() {
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [subject, setSubject] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const [draftStatus, setDraftStatus] = useState<'idle' | 'saving' | 'saved'>('idle');
  const draftTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

      await sendMessage({
        to: to.split(',').map((s) => s.trim()).filter(Boolean),
        cc: cc ? cc.split(',').map((s) => s.trim()).filter(Boolean) : undefined,
        subject,
        html_body: htmlBody,
        text_body: textBody,
      });

      setViewMode('list');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send');
    } finally {
      setSending(false);
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
      </div>
    </div>
  );
}
