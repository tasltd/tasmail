// TMAIL-331: Modern UI signatures pane. Lists every signature on the
// current account, lets the user create / edit / delete one, and toggle
// which one is the default. The default is what ComposeModal injects into
// new messages — see SignaturesPanel.test or e2e/specs/modern-ui-signatures
// for the round-trip proof.
//
// Editor: TipTap (StarterKit + Link + Placeholder) so the HTML body is
// rich-text and stays consistent with what the compose modal accepts. The
// plain-text body is a sibling <textarea> so users running text-only
// clients still see a useful signature.
//
// The pane is mounted by SettingsPage when the `signatures` tab is active
// (see tabs.ts → SettingsTab.component) so adding it required zero route
// changes — the tab registry is the single source of truth.
import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Link from '@tiptap/extension-link';
import Placeholder from '@tiptap/extension-placeholder';
import {
  Bold,
  Check,
  Edit2,
  Italic,
  Link as LinkIcon,
  List,
  PenSquare,
  Plus,
  Star,
  StarOff,
  Trash2,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Textarea } from '@/components/ui/textarea';
import {
  createSignature,
  deleteSignature,
  fetchSignatures,
  updateSignature,
  type Signature,
} from '@/api/signatures';

// Stable key for the React Query cache — also used by ComposeModal so the
// signature list invalidation here triggers compose to pick up renamed /
// re-defaulted entries on the next mount.
export const SIGNATURES_QUERY_KEY = ['signatures'] as const;

interface DraftSignature {
  name: string;
  html_body: string;
  text_body: string;
  is_default: boolean;
}

export function SignaturesPanel() {
  const queryClient = useQueryClient();
  const [editingId, setEditingId] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const { data: signatures, isLoading } = useQuery({
    queryKey: SIGNATURES_QUERY_KEY,
    queryFn: fetchSignatures,
  });

  const invalidateSignatures = () =>
    queryClient.invalidateQueries({ queryKey: SIGNATURES_QUERY_KEY });

  const createMut = useMutation({
    mutationFn: createSignature,
    onSuccess: () => {
      invalidateSignatures();
      setIsCreating(false);
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'Failed to create signature.'),
  });

  const updateMut = useMutation({
    mutationFn: ({
      id,
      data,
    }: {
      id: string;
      data: Parameters<typeof updateSignature>[1];
    }) => updateSignature(id, data),
    onSuccess: () => {
      invalidateSignatures();
      setEditingId(null);
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'Failed to save signature.'),
  });

  const deleteMut = useMutation({
    mutationFn: deleteSignature,
    onSuccess: () => {
      invalidateSignatures();
      setError(null);
    },
    onError: (e: Error) => setError(e.message || 'Failed to delete signature.'),
  });

  const editingSignature = signatures?.find((s) => s.id === editingId) ?? null;
  const showEditor = isCreating || Boolean(editingSignature);

  const handleCreateClick = () => {
    setIsCreating(true);
    setEditingId(null);
    setError(null);
  };

  const handleEditClick = (sig: Signature) => {
    setEditingId(sig.id);
    setIsCreating(false);
    setError(null);
  };

  const handleEditorSave = (draft: DraftSignature) => {
    if (isCreating) {
      createMut.mutate(draft);
    } else if (editingId) {
      updateMut.mutate({ id: editingId, data: draft });
    }
  };

  const handleEditorCancel = () => {
    setIsCreating(false);
    setEditingId(null);
    setError(null);
  };

  const handleSetDefault = (sig: Signature) => {
    if (sig.is_default) return;
    updateMut.mutate({ id: sig.id, data: { is_default: true } });
  };

  const handleDelete = (sig: Signature) => {
    if (
      !window.confirm(
        `Delete signature "${sig.name}"? This cannot be undone.`,
      )
    ) {
      return;
    }
    deleteMut.mutate(sig.id);
  };

  return (
    <div
      data-testid="settings-tab-signatures-pane"
      className="h-full w-full p-6 sm:p-8 overflow-y-auto"
    >
      <header className="flex items-center justify-between gap-3 mb-2 flex-wrap">
        <div className="flex items-center gap-3">
          <PenSquare
            className="size-6 text-blue-600 dark:text-blue-400"
            aria-hidden="true"
          />
          <h2 className="text-xl sm:text-2xl font-semibold">Signatures</h2>
        </div>
        <Button
          onClick={handleCreateClick}
          disabled={showEditor}
          data-testid="signatures-new-button"
        >
          <Plus className="size-4" />
          New signature
        </Button>
      </header>
      <p className="text-sm text-zinc-600 dark:text-zinc-400 max-w-2xl mb-6">
        HTML and plain-text signatures attached to outgoing mail. The signature
        marked <strong>Default</strong> is prepended automatically when you
        compose a new message.
      </p>

      {error && (
        <div
          role="alert"
          data-testid="signatures-error"
          className="mb-4 rounded-md border border-red-300 bg-red-50 px-3 py-2 text-sm text-red-700 dark:border-red-800 dark:bg-red-950 dark:text-red-200"
        >
          {error}
        </div>
      )}

      {showEditor && (
        <SignatureEditor
          key={editingSignature?.id ?? 'new'}
          initial={editingSignature ?? null}
          isSaving={createMut.isPending || updateMut.isPending}
          onSave={handleEditorSave}
          onCancel={handleEditorCancel}
        />
      )}

      <section data-testid="signatures-list" className="mt-6">
        {isLoading && (
          <p
            data-testid="signatures-loading"
            className="text-sm text-zinc-500"
          >
            Loading signatures…
          </p>
        )}
        {!isLoading && (!signatures || signatures.length === 0) && !showEditor && (
          <div
            data-testid="signatures-empty"
            className="rounded-lg border border-dashed border-zinc-300 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900/40 p-6 text-sm text-zinc-500"
          >
            <p className="font-medium text-zinc-700 dark:text-zinc-300 mb-1">
              No signatures yet
            </p>
            <p>Click "New signature" to create one.</p>
          </div>
        )}
        {signatures?.map((sig) => (
          <SignatureRow
            key={sig.id}
            signature={sig}
            disabled={
              showEditor || updateMut.isPending || deleteMut.isPending
            }
            onEdit={() => handleEditClick(sig)}
            onDelete={() => handleDelete(sig)}
            onSetDefault={() => handleSetDefault(sig)}
          />
        ))}
      </section>
    </div>
  );
}

interface SignatureRowProps {
  signature: Signature;
  disabled: boolean;
  onEdit: () => void;
  onDelete: () => void;
  onSetDefault: () => void;
}

function SignatureRow({
  signature,
  disabled,
  onEdit,
  onDelete,
  onSetDefault,
}: SignatureRowProps) {
  return (
    <div
      data-testid={`signature-row-${signature.id}`}
      data-default={signature.is_default ? 'true' : 'false'}
      className="flex items-start gap-3 p-3 border-b border-zinc-200 dark:border-zinc-800 last:border-b-0"
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <strong className="text-sm">{signature.name}</strong>
          {signature.is_default && (
            <span
              data-testid={`signature-default-badge-${signature.id}`}
              className="text-[10px] font-semibold uppercase tracking-wider bg-blue-600 text-white px-2 py-0.5 rounded-full"
            >
              Default
            </span>
          )}
        </div>
        <p
          data-testid={`signature-preview-${signature.id}`}
          className="text-xs text-zinc-500 dark:text-zinc-400 mt-1 truncate"
        >
          {signature.text_body.slice(0, 120) ||
            stripHtml(signature.html_body).slice(0, 120) ||
            '(empty body)'}
        </p>
      </div>
      <div className="flex items-center gap-1 shrink-0">
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          title={signature.is_default ? 'Already the default' : 'Set as default'}
          aria-label={
            signature.is_default ? 'Already the default' : 'Set as default'
          }
          data-testid={`signature-set-default-${signature.id}`}
          disabled={disabled || signature.is_default}
          onClick={onSetDefault}
        >
          {signature.is_default ? (
            <Star className="size-4 fill-current" />
          ) : (
            <StarOff className="size-4" />
          )}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-8"
          title="Edit"
          aria-label={`Edit signature ${signature.name}`}
          data-testid={`signature-edit-${signature.id}`}
          disabled={disabled}
          onClick={onEdit}
        >
          <Edit2 className="size-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-8 text-red-600 hover:text-red-700"
          title="Delete"
          aria-label={`Delete signature ${signature.name}`}
          data-testid={`signature-delete-${signature.id}`}
          disabled={disabled}
          onClick={onDelete}
        >
          <Trash2 className="size-4" />
        </Button>
      </div>
    </div>
  );
}

// Cheap HTML→text fallback used only for the preview line in the list. Not
// suitable for storing as text_body (no entity decoding) — but for a 120-char
// truncated preview it is fine and avoids dragging DOMPurify into the panel.
function stripHtml(html: string): string {
  return html.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
}

interface SignatureEditorProps {
  initial: Signature | null;
  isSaving: boolean;
  onSave: (draft: DraftSignature) => void;
  onCancel: () => void;
}

function SignatureEditor({
  initial,
  isSaving,
  onSave,
  onCancel,
}: SignatureEditorProps) {
  const [name, setName] = useState(initial?.name ?? '');
  const [textBody, setTextBody] = useState(initial?.text_body ?? '');
  const [isDefault, setIsDefault] = useState(initial?.is_default ?? false);
  const [editorTick, setEditorTick] = useState(0);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Link.configure({ openOnClick: false, autolink: false }),
      Placeholder.configure({
        placeholder: 'Best regards,\nYour Name',
      }),
    ],
    content: initial?.html_body || '',
    editorProps: {
      attributes: {
        class:
          'tasmail-sig-editor min-h-[120px] focus:outline-none prose prose-sm dark:prose-invert max-w-none px-3 py-2',
        'data-testid': 'signature-rte-editor',
      },
    },
    onUpdate: () => setEditorTick((t) => t + 1),
    onSelectionUpdate: () => setEditorTick((t) => t + 1),
  });

  // Sync the editor when switching between New / Edit on the same mounted
  // instance. The component is keyed by editingId in the parent so this
  // mostly fires on first mount, but the guard handles a future case where
  // someone passes a different `initial` without changing the key.
  useEffect(() => {
    if (!editor) return;
    const current = editor.getHTML();
    const next = initial?.html_body || '';
    if (current !== next) {
      editor.commands.setContent(next, { emitUpdate: false });
    }
  }, [editor, initial]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    const htmlBody = editor && !editor.isEmpty ? editor.getHTML() : '';
    onSave({
      name: name.trim(),
      html_body: htmlBody,
      text_body: textBody,
      is_default: isDefault,
    });
  };

  return (
    <form
      data-testid="signature-editor"
      onSubmit={handleSubmit}
      className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-zinc-50/50 dark:bg-zinc-900/40 p-4 sm:p-5 space-y-4"
    >
      <h3 className="text-base font-semibold">
        {initial ? 'Edit signature' : 'New signature'}
      </h3>

      <div className="space-y-1">
        <label
          htmlFor="signature-name"
          className="text-sm font-medium text-zinc-700 dark:text-zinc-300"
        >
          Name
        </label>
        <Input
          id="signature-name"
          data-testid="signature-name-input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="e.g. Work, Personal"
          required
          maxLength={200}
        />
      </div>

      <div className="space-y-1">
        <span className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
          HTML body
        </span>
        <div
          className="rounded-md border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-950 overflow-hidden"
          data-editor-tick={editorTick}
        >
          <div className="flex items-center gap-1 px-2 py-1 border-b border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900/40">
            <Button
              type="button"
              variant={editor?.isActive('bold') ? 'secondary' : 'ghost'}
              size="icon"
              className="size-7"
              title="Bold"
              aria-label="Bold"
              aria-pressed={editor?.isActive('bold') ?? false}
              data-testid="signature-rte-bold"
              disabled={!editor}
              onClick={() => editor?.chain().focus().toggleBold().run()}
            >
              <Bold className="size-3.5" />
            </Button>
            <Button
              type="button"
              variant={editor?.isActive('italic') ? 'secondary' : 'ghost'}
              size="icon"
              className="size-7"
              title="Italic"
              aria-label="Italic"
              aria-pressed={editor?.isActive('italic') ?? false}
              data-testid="signature-rte-italic"
              disabled={!editor}
              onClick={() => editor?.chain().focus().toggleItalic().run()}
            >
              <Italic className="size-3.5" />
            </Button>
            <Button
              type="button"
              variant={editor?.isActive('link') ? 'secondary' : 'ghost'}
              size="icon"
              className="size-7"
              title="Insert link"
              aria-label="Insert link"
              aria-pressed={editor?.isActive('link') ?? false}
              data-testid="signature-rte-link"
              disabled={!editor}
              onClick={() => {
                if (!editor) return;
                if (editor.isActive('link')) {
                  editor.chain().focus().unsetLink().run();
                  return;
                }
                const previous =
                  (editor.getAttributes('link').href as string | undefined) ??
                  '';
                const url = window.prompt('Enter URL', previous);
                if (url === null) return;
                if (url === '') {
                  editor.chain().focus().unsetLink().run();
                  return;
                }
                if (/^\s*javascript:/i.test(url)) return;
                const { from, to: rangeTo } = editor.state.selection;
                if (from !== rangeTo) {
                  editor
                    .chain()
                    .focus()
                    .extendMarkRange('link')
                    .setLink({ href: url })
                    .run();
                  return;
                }
                const safe = url.replace(/"/g, '&quot;').replace(/</g, '&lt;');
                editor
                  .chain()
                  .focus()
                  .insertContent(`<a href="${safe}">${safe}</a>`)
                  .run();
              }}
            >
              <LinkIcon className="size-3.5" />
            </Button>
            <Button
              type="button"
              variant={editor?.isActive('bulletList') ? 'secondary' : 'ghost'}
              size="icon"
              className="size-7"
              title="Bullet list"
              aria-label="Bullet list"
              aria-pressed={editor?.isActive('bulletList') ?? false}
              data-testid="signature-rte-list"
              disabled={!editor}
              onClick={() =>
                editor?.chain().focus().toggleBulletList().run()
              }
            >
              <List className="size-3.5" />
            </Button>
          </div>
          <EditorContent editor={editor} />
        </div>
      </div>

      <div className="space-y-1">
        <label
          htmlFor="signature-text-body"
          className="text-sm font-medium text-zinc-700 dark:text-zinc-300"
        >
          Plain-text body
        </label>
        <Textarea
          id="signature-text-body"
          data-testid="signature-text-input"
          value={textBody}
          onChange={(e) => setTextBody(e.target.value)}
          placeholder="Best regards,&#10;Your Name"
          rows={4}
        />
        <p className="text-xs text-zinc-500">
          Used for recipients whose clients can't render HTML.
        </p>
      </div>

      <label className="inline-flex items-center gap-2 text-sm">
        <input
          type="checkbox"
          data-testid="signature-default-checkbox"
          checked={isDefault}
          onChange={(e) => setIsDefault(e.target.checked)}
          className="size-4"
        />
        Set as default signature
      </label>

      <div className="flex items-center gap-2 pt-2">
        <Button
          type="submit"
          data-testid="signature-save-button"
          disabled={isSaving || !name.trim()}
        >
          <Check className="size-4" />
          {isSaving ? 'Saving…' : 'Save'}
        </Button>
        <Button
          type="button"
          variant="ghost"
          onClick={onCancel}
          data-testid="signature-cancel-button"
          disabled={isSaving}
        >
          <X className="size-4" />
          Cancel
        </Button>
      </div>
    </form>
  );
}
