import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { X, Minimize2, Maximize2, Paperclip, Send, Save, Bold, Italic, Link as LinkIcon, List } from 'lucide-react';
// TMAIL-330: TipTap powers the rich-text body. StarterKit covers paragraphs,
// bold, italic, lists and headings; Link is opt-in (we want the toolbar to
// prompt for a URL rather than auto-linkify pasted text); Placeholder renders
// the "Compose your message…" hint that the old <Textarea> used to.
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { scheduledApi } from '@/api/scheduled';
import { attachmentsApi, type Attachment } from '@/api/attachments';
import { saveDraft } from '@/api/messages';
// TMAIL-331: pull the user's signatures so we can prepend the default one
// onto fresh-compose bodies. Replies/forwards keep the user's signature out
// of the way of the quoted block — that matches Gmail/Outlook UX. The
// signatures query is shared with SignaturesPanel via SIGNATURES_QUERY_KEY
// so a save/delete in settings invalidates this query too.
import {
  fetchSignatures,
  pickDefaultSignature,
} from '@/api/signatures';
import { SIGNATURES_QUERY_KEY } from '@/features/settings/SignaturesPanel';
import type { ReplyContext } from './replyContext';

// TMAIL-321: 25 MB total compose limit, matching the backend's
// storage.max_file_size default. Extracted so the same number drives both
// the "Attachments 12 MB / 25 MB" label and the pre-send guard.
const MAX_TOTAL_ATTACHMENT_BYTES = 25 * 1024 * 1024;

interface ComposeModalProps {
  isOpen: boolean;
  onClose: () => void;
  // Added: TMAIL-319 — Reply / Reply All / Forward prefill payload built by
  // EmailReader → buildReplyContext(). When non-null the modal opens with
  // the recipients, subject (Re: / Fwd: prefix), and quoted body already
  // populated, and stamps the In-Reply-To / References headers onto the
  // outbound /api/messages/schedule request so downstream mail clients
  // thread the conversation correctly (RFC 5322 §3.6.4).
  // Null means a blank compose-from-scratch.
  replyContext?: ReplyContext | null;
}

// TMAIL-219: send via scheduledApi.scheduleSend so the modern UI uses the
// same code path the production SPA's composer does. delay_seconds=0 means
// "send immediately" for now — the modern UI doesn't have an undo banner
// yet; that can ship later if desired.
function splitAddrs(s: string): string[] {
  return s.split(/[,;]/).map((x) => x.trim()).filter(Boolean);
}

// TMAIL-330: minimal HTML escape used when seeding the TipTap doc from the
// plain-text quoted block built by buildReplyContext(). Keeps `<`, `>`, `&`
// and quotes from being interpreted as markup when the reply body itself
// contains text like "foo <bar@baz>".
function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// TMAIL-321: each attachment row tracks both the local File (for size + name
// display) and the server-side upload state. We immediately upload on file
// select so by the time the user hits Send we already have an attachment_id
// to pass to the schedule API. `error` lets the row render a retry control.
type UploadStatus = 'uploading' | 'uploaded' | 'error';
interface ComposeAttachment {
  // Stable client-side key so React can keep list identity across re-renders.
  key: string;
  file: File;
  status: UploadStatus;
  // Populated once the upload completes — used as the `attachment_ids` payload.
  serverId: string | null;
  // Captured upload error message; shown inline so the user knows which row
  // is broken and can remove it before retrying Send.
  errorMessage: string | null;
}

function makeAttachmentKey(file: File): string {
  // Random + name keeps duplicates with the same filename distinguishable
  // and avoids leaking sensitive paths into the DOM.
  return `${file.name}-${file.size}-${Math.random().toString(36).slice(2)}`;
}

export function ComposeModal({ isOpen, onClose, replyContext = null }: ComposeModalProps) {
  const queryClient = useQueryClient();
  const [minimized, setMinimized] = useState(false);
  const [showCc, setShowCc] = useState(false);
  const [showBcc, setShowBcc] = useState(false);
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [bcc, setBcc] = useState('');
  const [subject, setSubject] = useState('');
  // TMAIL-331: fetch the user's signatures so we can seed the editor with the
  // default one. `enabled: isOpen` means we don't ping /api/signatures until
  // the user actually opens the composer, and shared cache with the settings
  // pane means a save there is reflected immediately the next time the modal
  // opens.
  const { data: signatures } = useQuery({
    queryKey: SIGNATURES_QUERY_KEY,
    queryFn: fetchSignatures,
    enabled: isOpen,
    staleTime: 60_000,
  });
  const defaultSignature = pickDefaultSignature(signatures);
  // TMAIL-330: `body` no longer lives in React state — the TipTap editor owns
  // the document. We keep a render-tick counter so toolbar active-states and
  // Send/Save Draft disabled-checks re-evaluate after each editor transaction
  // (TipTap doesn't trigger React re-renders by itself).
  const [editorTick, setEditorTick] = useState(0);
  const [error, setError] = useState<string | null>(null);
  // TMAIL-321: composer-local attachment list (with upload state) — see
  // ComposeAttachment above. Replaces the old File[] which never made it to
  // the wire.
  const [attachments, setAttachments] = useState<ComposeAttachment[]>([]);

  // TMAIL-330: TipTap editor configured to match the classic SPA composer
  // (StarterKit + Link + Placeholder). Each transaction bumps `editorTick`
  // so toolbar active-states / Send disabled-checks update — TipTap's
  // editor.isActive() / editor.isEmpty are not reactive on their own.
  const editor = useEditor({
    extensions: [
      StarterKit,
      // openOnClick=false so clicking a link inside the editor doesn't
      // navigate away mid-compose. The reader UI handles link clicks.
      // autolink=false so TipTap doesn't re-derive an href from the text
      // when the user inserts a link with an explicit URL via the toolbar
      // (otherwise typing "example.com" inside the link mark would
      // overwrite the user-supplied href to "http://example.com").
      Link.configure({ openOnClick: false, autolink: false }),
      Placeholder.configure({ placeholder: 'Compose your message...' }),
    ],
    content: '',
    editorProps: {
      attributes: {
        // tiptap renders ProseMirror inside this <div>; the styles match the
        // surrounding modal padding and give the placeholder a min-height so
        // the editor feels like the old textarea.
        class:
          'tasmail-rte-editor min-h-[150px] sm:min-h-[250px] focus:outline-none prose prose-sm dark:prose-invert max-w-none',
        'data-testid': 'compose-rte-editor',
      },
    },
    onUpdate: () => {
      setEditorTick((t) => t + 1);
    },
    onSelectionUpdate: () => {
      setEditorTick((t) => t + 1);
    },
  });

  // Added: TMAIL-319 — re-seed the form state from `replyContext` whenever
  // the modal opens (or the source message changes). Using an effect rather
  // than `useState(() => fromReplyCtx)` so flipping between Reply / Reply All
  // / Forward on the same modal instance re-prefills correctly. Reveals Cc
  // automatically when the prefill includes any Cc addresses so the user
  // sees them without hunting for the toggle.
  // TMAIL-330: this effect must NOT depend on `editor` — useEditor() returns
  // null on the first render and the Editor on the next, so listing `editor`
  // here would cause the effect to fire a second time once the editor mounts
  // and wipe anything the user has typed in the meantime. The editor content
  // seeding lives in its own effect below, which is allowed to re-run when
  // the editor instance arrives.
  useEffect(() => {
    if (!isOpen) return;
    if (replyContext) {
      setTo(replyContext.to.join(', '));
      setCc(replyContext.cc.join(', '));
      setBcc('');
      setSubject(replyContext.subject);
      setShowCc(replyContext.cc.length > 0);
      setShowBcc(false);
    } else {
      setTo('');
      setCc('');
      setBcc('');
      setSubject('');
      setShowCc(false);
      setShowBcc(false);
    }
    setError(null);
    setAttachments([]);
  }, [isOpen, replyContext]);

  // TMAIL-330: keep the TipTap doc in sync with the open/replyContext state.
  // Runs separately so that the editor mounting (null → Editor) doesn't drop
  // the user's already-typed form fields above.
  //
  // TMAIL-331: on fresh compose (no replyContext) prepend the user's default
  // signature so they don't have to retype it every time. A leading <p></p>
  // gives them a blank paragraph above the signature to start typing into
  // without accidentally erasing the signature mark on first keystroke. We
  // intentionally do NOT inject the signature on reply/forward — the quoted
  // block already occupies the body and Gmail/Outlook UX puts the signature
  // above the quote only when the user composes from scratch.
  useEffect(() => {
    if (!isOpen || !editor) return;
    if (replyContext) {
      const html = replyContext.body
        .split(/\r?\n/)
        .map((line) => (line.length === 0 ? '<p></p>' : `<p>${escapeHtml(line)}</p>`))
        .join('');
      editor.commands.setContent(html, { emitUpdate: false });
    } else if (defaultSignature) {
      const sigHtml = defaultSignature.html_body
        ? defaultSignature.html_body
        : defaultSignature.text_body
          ? defaultSignature.text_body
              .split(/\r?\n/)
              .map((line) =>
                line.length === 0 ? '<p></p>' : `<p>${escapeHtml(line)}</p>`,
              )
              .join('')
          : '';
      const wrapped = sigHtml
        ? `<p></p><div data-tasmail-signature="true">${sigHtml}</div>`
        : '';
      editor.commands.setContent(wrapped, { emitUpdate: false });
    } else {
      editor.commands.clearContent(false);
    }
    // defaultSignature is derived from the React Query result so it can
    // arrive a tick after the editor mounts. Re-running this effect when it
    // arrives is what makes the signature appear on the first compose of the
    // session without a manual reopen.
  }, [isOpen, replyContext, editor, defaultSignature]);

  const sendMut = useMutation({
    mutationFn: async () => {
      // TMAIL-321: enforce both gates before any HTTP round-trip:
      //   1. every attachment must be successfully uploaded (have a serverId);
      //      half-uploaded files would silently disappear from the outbound
      //      message otherwise.
      //   2. the running total must be under the 25 MB cap (re-checked here
      //      because users can paste long bodies that push them over later).
      const pending = attachments.find((a) => a.status !== 'uploaded' || !a.serverId);
      if (pending) {
        if (pending.status === 'uploading') {
          throw new Error('Wait for attachment uploads to finish before sending.');
        }
        throw new Error(
          `Attachment "${pending.file.name}" failed to upload — remove or retry it before sending.`,
        );
      }
      const total = attachments.reduce((sum, a) => sum + a.file.size, 0);
      if (total > MAX_TOTAL_ATTACHMENT_BYTES) {
        throw new Error('Attachments exceed the 25 MB limit. Remove some files and try again.');
      }
      const attachmentIds = attachments
        .map((a) => a.serverId)
        .filter((id): id is string => Boolean(id));

      // TMAIL-330: the editor is the source of truth for the body. We send
      // both the rendered HTML (so recipients see formatting) and the
      // plain-text fallback (for clients that only render text/plain or for
      // accessibility/spam-score scanners that prefer it). An empty doc gives
      // editor.getHTML() = "<p></p>" — treat that as "no body" so we don't
      // emit a phantom MIME part.
      const htmlBody = editor && !editor.isEmpty ? editor.getHTML() : '';
      const textBody = editor ? editor.getText() : '';

      return scheduledApi.scheduleSend({
        to: splitAddrs(to),
        cc: cc.trim() ? splitAddrs(cc) : undefined,
        bcc: bcc.trim() ? splitAddrs(bcc) : undefined,
        subject,
        html_body: htmlBody || undefined,
        text_body: textBody || undefined,
        delay_seconds: 0,
        // TMAIL-319: forward the threading headers from the open replyContext
        // so the backend can persist them and the email scheduler can stamp
        // In-Reply-To / References on the outbound message. Blank for a fresh
        // compose so we don't emit phantom headers.
        in_reply_to: replyContext?.inReplyTo ?? undefined,
        references:
          replyContext && replyContext.references.length > 0
            ? replyContext.references
            : undefined,
        // TMAIL-321: pass the uploaded attachment IDs so the backend links
        // them to the scheduled row and the email_scheduler emits them as
        // MIME parts on the outbound message.
        attachment_ids: attachmentIds.length > 0 ? attachmentIds : undefined,
      });
    },
    onSuccess: () => {
      // Sent message lands in the user's Sent folder; bump folder counts so
      // the sidebar's unread badges refresh.
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      queryClient.invalidateQueries({ queryKey: ['messages'] });
      // Reset the form for the next compose.
      setTo(''); setCc(''); setBcc(''); setSubject('');
      editor?.commands.clearContent(false);
      setAttachments([]);
      setError(null);
      onClose();
    },
    onError: (err: Error) => setError(err.message || 'Send failed.'),
  });

  // TMAIL-238: Save Draft → POST /api/drafts. Backend appends an RFC-822
  // message to Dovecot's Drafts folder with the \Draft flag set, so it
  // shows up in the Drafts list without going through the SMTP queue.
  // TMAIL-330: ship html_body + text_body sourced from the TipTap doc so
  // the user's formatting survives a draft round-trip.
  const draftMut = useMutation({
    mutationFn: () => {
      const htmlBody = editor && !editor.isEmpty ? editor.getHTML() : '';
      const textBody = editor ? editor.getText() : '';
      return saveDraft({
        to: splitAddrs(to),
        cc: cc.trim() ? splitAddrs(cc) : undefined,
        subject: subject || '(no subject)',
        html_body: htmlBody || undefined,
        text_body: textBody || undefined,
        // TMAIL-319: include the same threading headers on drafts so a draft
        // started from Reply / Reply All / Forward keeps its conversation
        // identity. The backend may not persist them yet — the modal stays
        // forward-compatible without a separate branch.
        in_reply_to: replyContext?.inReplyTo ?? undefined,
        references:
          replyContext && replyContext.references.length > 0
            ? replyContext.references
            : undefined,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['folders'] });
      queryClient.invalidateQueries({ queryKey: ['messages', 'Drafts'] });
      setError(null);
      onClose();
    },
    onError: (err: Error) => setError(err.message || 'Save draft failed.'),
  });

  if (!isOpen) return null;

  // TMAIL-321: each newly-selected file is queued with a stable client-side
  // key, then immediately uploaded to /api/attachments in parallel. The row
  // stays visible while uploading so the user gets immediate feedback that
  // the file was accepted. Upload failures flip the row into `error` state
  // so the user can remove or retry without blocking the rest of the
  // attachments.
  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files) return;
    const incoming = Array.from(files).map<ComposeAttachment>((file) => ({
      key: makeAttachmentKey(file),
      file,
      status: 'uploading',
      serverId: null,
      errorMessage: null,
    }));
    setAttachments((prev) => [...prev, ...incoming]);
    // Reset the input so re-selecting the same file fires onChange again.
    e.target.value = '';

    incoming.forEach((row) => uploadAttachment(row));
  };

  const uploadAttachment = async (row: ComposeAttachment) => {
    try {
      const uploaded: Attachment = await attachmentsApi.upload(row.file);
      setAttachments((prev) =>
        prev.map((a) =>
          a.key === row.key
            ? { ...a, status: 'uploaded', serverId: uploaded.id, errorMessage: null }
            : a,
        ),
      );
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Upload failed — please retry.';
      setAttachments((prev) =>
        prev.map((a) =>
          a.key === row.key
            ? { ...a, status: 'error', serverId: null, errorMessage: message }
            : a,
        ),
      );
    }
  };

  const removeAttachment = (key: string) => {
    setAttachments((prev) => {
      const row = prev.find((a) => a.key === key);
      // Best-effort: if the file was already uploaded, ask the backend to
      // clean it up so abandoned uploads don't accumulate against the user's
      // storage quota. Errors here are intentionally swallowed because the
      // composer-local removal is what matters to the user — the daily
      // attachment quota sweep will catch any DB orphan.
      if (row?.serverId) {
        attachmentsApi.delete(row.serverId).catch(() => {});
      }
      return prev.filter((a) => a.key !== key);
    });
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
  };

  const totalSize = attachments.reduce((sum, a) => sum + a.file.size, 0);
  const maxSize = MAX_TOTAL_ATTACHMENT_BYTES;
  // TMAIL-321: any uploading / errored row blocks the Send button so the user
  // can't accidentally fire off a message minus the files they just attached.
  const hasUploadingAttachment = attachments.some((a) => a.status === 'uploading');
  const hasFailedAttachment = attachments.some((a) => a.status === 'error');
  const overTotalLimit = totalSize > maxSize;

  if (minimized) {
    return (
      <div className="fixed bottom-0 right-0 sm:right-4 w-full sm:w-80 bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 rounded-t-lg shadow-2xl z-50">
        <div className="flex items-center justify-between p-3 border-b border-zinc-200 dark:border-zinc-800 cursor-pointer hover:bg-zinc-50 dark:hover:bg-zinc-800" onClick={() => setMinimized(false)}>
          {/* Added: TMAIL-319 — modal heading reflects the active intent so
              the user (and screen readers) know whether this is a fresh
              compose, a Reply, a Reply All, or a Forward. */}
          <span className="font-medium">
            {replyContext == null && 'New Message'}
            {replyContext?.kind === 'reply' && 'Reply'}
            {replyContext?.kind === 'replyAll' && 'Reply All'}
            {replyContext?.kind === 'forward' && 'Forward'}
          </span>
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="icon" className="size-8">
              <Maximize2 className="size-4" />
            </Button>
            <Button variant="ghost" size="icon" className="size-8" onClick={(e) => { e.stopPropagation(); onClose(); }}>
              <X className="size-4" />
            </Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 sm:inset-auto sm:bottom-0 sm:right-4 sm:w-[560px] bg-white dark:bg-zinc-900 border border-zinc-200 dark:border-zinc-800 sm:rounded-t-lg shadow-2xl flex flex-col sm:max-h-[600px] z-50">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800">
        <span className="font-medium">New Message</span>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="size-8" onClick={() => setMinimized(true)}>
            <Minimize2 className="size-4" />
          </Button>
          <Button variant="ghost" size="icon" className="size-8" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>
      </div>

      {/* Recipients */}
      <div className="border-b border-zinc-200 dark:border-zinc-800">
        <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
          <span className="text-sm text-zinc-500 w-16">To</span>
          <Input
            type="text"
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="alice@example.com, bob@example.com"
            className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
          />
          <div className="flex gap-2 text-sm">
            <button onClick={() => setShowCc(!showCc)} className="text-blue-600 hover:underline">Cc</button>
            <button onClick={() => setShowBcc(!showBcc)} className="text-blue-600 hover:underline">Bcc</button>
          </div>
        </div>

        {showCc && (
          <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
            <span className="text-sm text-zinc-500 w-16">Cc</span>
            <Input
              type="text"
              value={cc}
              onChange={(e) => setCc(e.target.value)}
              placeholder="Carbon copy"
              className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
            />
          </div>
        )}

        {showBcc && (
          <div className="flex items-center px-3 py-2 border-b border-zinc-200 dark:border-zinc-800">
            <span className="text-sm text-zinc-500 w-16">Bcc</span>
            <Input
              type="text"
              value={bcc}
              onChange={(e) => setBcc(e.target.value)}
              placeholder="Blind carbon copy"
              className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
            />
          </div>
        )}

        <div className="flex items-center px-3 py-2">
          <span className="text-sm text-zinc-500 w-16">Subject</span>
          <Input
            type="text"
            value={subject}
            onChange={(e) => setSubject(e.target.value)}
            placeholder="Subject"
            className="flex-1 border-0 focus-visible:ring-0 focus-visible:ring-offset-0"
          />
        </div>
      </div>

      {/* Rich Text Toolbar — TMAIL-330: wired to TipTap commands. Active
          state mirrors editor.isActive() so the user sees which mark is on
          at the cursor. The `disabled` guard prevents toolbar clicks before
          the editor finishes mounting. */}
      <div className="flex items-center gap-1 px-3 py-2 border-b border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800/50">
        <Button
          variant={editor?.isActive('bold') ? 'secondary' : 'ghost'}
          size="icon"
          className="size-8"
          title="Bold"
          aria-label="Bold"
          aria-pressed={editor?.isActive('bold') ?? false}
          data-testid="compose-rte-bold"
          disabled={!editor}
          onClick={() => editor?.chain().focus().toggleBold().run()}
        >
          <Bold className="size-4" />
        </Button>
        <Button
          variant={editor?.isActive('italic') ? 'secondary' : 'ghost'}
          size="icon"
          className="size-8"
          title="Italic"
          aria-label="Italic"
          aria-pressed={editor?.isActive('italic') ?? false}
          data-testid="compose-rte-italic"
          disabled={!editor}
          onClick={() => editor?.chain().focus().toggleItalic().run()}
        >
          <Italic className="size-4" />
        </Button>
        <Button
          variant={editor?.isActive('link') ? 'secondary' : 'ghost'}
          size="icon"
          className="size-8"
          title="Insert link"
          aria-label="Insert link"
          aria-pressed={editor?.isActive('link') ?? false}
          data-testid="compose-rte-link"
          disabled={!editor}
          onClick={() => {
            if (!editor) return;
            if (editor.isActive('link')) {
              editor.chain().focus().unsetLink().run();
              return;
            }
            const previous = (editor.getAttributes('link').href as string | undefined) ?? '';
            // Native prompt — matches the classic composer's behavior; a
            // proper modal is tracked separately. Cancel = no-op so the user
            // can back out without dropping the selection.
            const url = window.prompt('Enter URL', previous);
            if (url === null) return;
            if (url === '') {
              editor.chain().focus().unsetLink().run();
              return;
            }
            // TipTap normalizes the href; we only block obviously unsafe
            // schemes here so the toolbar doesn't become a javascript:
            // injection vector.
            if (/^\s*javascript:/i.test(url)) return;
            const { from, to: rangeTo } = editor.state.selection;
            const hasSelection = from !== rangeTo;
            if (hasSelection) {
              // Selection exists — wrap it (or extend the existing link mark)
              // in the new href.
              editor.chain().focus().extendMarkRange('link').setLink({ href: url }).run();
              return;
            }
            // No selection — insert the URL itself as the visible link text
            // wrapped in an <a> node. This is what Gmail does when the user
            // clicks "Insert link" without preselecting text: the URL doubles
            // as the display text, and the user can edit it inline afterwards.
            const safeUrl = url.replace(/"/g, '&quot;').replace(/</g, '&lt;');
            editor
              .chain()
              .focus()
              .insertContent(`<a href="${safeUrl}">${safeUrl}</a>`)
              .run();
          }}
        >
          <LinkIcon className="size-4" />
        </Button>
        <Button
          variant={editor?.isActive('bulletList') ? 'secondary' : 'ghost'}
          size="icon"
          className="size-8"
          title="Bullet list"
          aria-label="Bullet list"
          aria-pressed={editor?.isActive('bulletList') ?? false}
          data-testid="compose-rte-bullet-list"
          disabled={!editor}
          onClick={() => editor?.chain().focus().toggleBulletList().run()}
        >
          <List className="size-4" />
        </Button>
        <div className="flex-1" />
        <label htmlFor="file-upload">
          <Button variant="ghost" size="icon" className="size-8" title="Attach files" asChild>
            <span>
              <Paperclip className="size-4" />
            </span>
          </Button>
        </label>
        <input
          id="file-upload"
          type="file"
          multiple
          onChange={handleFileSelect}
          className="hidden"
        />
      </div>

      {/* Message Body — TMAIL-330: TipTap editor (StarterKit + Link +
          Placeholder). EditorContent renders the ProseMirror DOM; styles
          live inside the editorProps.attributes.class above so the wrapper
          here is just a scroll container. */}
      <div className="flex-1 p-3 overflow-y-auto">
        <div
          data-testid="compose-rte-wrapper"
          // TMAIL-330: `data-editor-tick` keeps the editorTick state used by
          // toolbar isActive() reads (TipTap doesn't trigger React updates on
          // its own). Without this attribute the tsc unused-variable check
          // would fire.
          data-editor-tick={editorTick}
          className="min-h-[150px] sm:min-h-[250px]"
        >
          <EditorContent editor={editor} />
        </div>
        {error && (
          <div role="alert" className="mt-2 text-sm text-red-600">{error}</div>
        )}

        {/* TMAIL-321: each attachment row reflects upload state — `Uploading…`
            while in flight, the size when finished, an inline error message
            (with a retry link) if the upload failed. The Send button below
            stays disabled until every row is in `uploaded`. */}
        {attachments.length > 0 && (
          <div className="mt-4 space-y-2" data-testid="compose-attachments">
            <div className="flex items-center justify-between text-sm">
              <span className="font-medium">Attachments</span>
              <span className={overTotalLimit ? 'text-red-600' : 'text-zinc-500'}>
                {formatBytes(totalSize)} / 25 MB
              </span>
            </div>
            {attachments.map((row) => (
              <div
                key={row.key}
                data-testid={`compose-attachment-${row.status}`}
                className="flex items-center justify-between p-2 bg-zinc-50 dark:bg-zinc-800 rounded border border-zinc-200 dark:border-zinc-700"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <Paperclip className="size-4 text-zinc-500 flex-shrink-0" />
                  <span className="text-sm truncate">{row.file.name}</span>
                  <span className="text-xs text-zinc-500 flex-shrink-0">
                    {formatBytes(row.file.size)}
                  </span>
                  {row.status === 'uploading' && (
                    <span className="text-xs text-blue-600 flex-shrink-0">Uploading…</span>
                  )}
                  {row.status === 'error' && (
                    <span className="text-xs text-red-600 flex-shrink-0">
                      {row.errorMessage || 'Upload failed'}
                      <button
                        type="button"
                        onClick={() => {
                          // Flip back to uploading so the indicator updates,
                          // then re-fire the upload for this row only.
                          setAttachments((prev) =>
                            prev.map((a) =>
                              a.key === row.key
                                ? { ...a, status: 'uploading', errorMessage: null }
                                : a,
                            ),
                          );
                          uploadAttachment(row);
                        }}
                        className="ml-2 underline"
                      >
                        Retry
                      </button>
                    </span>
                  )}
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  className="size-6 flex-shrink-0"
                  onClick={() => removeAttachment(row.key)}
                  aria-label={`Remove attachment ${row.file.name}`}
                >
                  <X className="size-3" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between p-3 border-t border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-800 shrink-0">
        <div className="flex gap-2">
          <Button
            className="bg-blue-600 hover:bg-blue-700"
            onClick={() => sendMut.mutate()}
            disabled={
              sendMut.isPending ||
              !to.trim() ||
              !subject.trim() ||
              // TMAIL-321: block Send while attachments are still in flight,
              // have failed to upload, or exceed the 25 MB cap. Each branch
              // prevents a different silent-failure mode where the user
              // thinks their files were sent but the wire payload didn't
              // carry them.
              hasUploadingAttachment ||
              hasFailedAttachment ||
              overTotalLimit
            }
            title={
              hasUploadingAttachment
                ? 'Waiting for attachment uploads to finish…'
                : hasFailedAttachment
                  ? 'One or more attachments failed to upload. Remove or retry them.'
                  : overTotalLimit
                    ? 'Attachments exceed the 25 MB limit.'
                    : undefined
            }
          >
            <Send className="size-4 mr-1 sm:mr-2" />
            {sendMut.isPending ? 'Sending…' : 'Send'}
          </Button>
          <Button
            variant="outline"
            // TMAIL-330: Save Draft is allowed once the user has typed
            // anything — recipients, subject, or body text in the editor.
            // `editor.isEmpty` covers the empty-doc case where the cursor
            // is the only content.
            disabled={
              draftMut.isPending ||
              (!to.trim() && !subject.trim() && (editor?.isEmpty ?? true))
            }
            onClick={() => draftMut.mutate()}
            title="Save as draft"
          >
            <Save className="size-4 mr-1 sm:mr-2" />
            <span className="hidden xs:inline sm:inline">
              {draftMut.isPending ? 'Saving…' : 'Save Draft'}
            </span>
            <span className="xs:hidden sm:hidden">
              {draftMut.isPending ? '…' : 'Save'}
            </span>
          </Button>
        </div>
        <Button variant="ghost" size="sm" onClick={onClose}>
          Discard
        </Button>
      </div>
    </div>
  );
}
