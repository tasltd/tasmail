// TMAIL-348: Per-message comments thread in the Modern UI EmailReader.
//
// Renders below the message body + attachments. Lists every internal comment
// attached to the (folder, uid) pair, lets the signed-in user post a new one,
// and gives Pencil / Trash actions on each existing entry.
//
// Mailbox-scoping note (matches `backend/src/handlers/comments.rs`):
//   The comments table is keyed by mailbox_id and protected by PostgreSQL RLS.
//   Every comment the user can read is by definition theirs — the
//   "edit/delete OWN" requirement from TMAIL-348 is enforced server-side, so
//   this component shows the edit + delete affordances on every row without
//   any client-side identity check. The backend rejects cross-mailbox writes
//   with a 404 even if the UI were tricked into asking.
//
// Styled with shadcn primitives (Button + Textarea) plus Tailwind utilities so
// it matches the rest of the Modern UI — see `SignaturesPanel.tsx` for the
// canonical CRUD pattern in this codebase.
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { MessageSquare, Pencil, Send, Trash2 } from 'lucide-react';
import { format } from 'date-fns';
import { Button } from '@/components/ui/button';
import { Textarea } from '@/components/ui/textarea';
import {
  createComment,
  deleteComment,
  fetchComments,
  updateComment,
  type EmailComment,
} from '@/api/comments';

interface CommentsThreadProps {
  folder: string;
  uid: number;
}

/**
 * Format an ISO timestamp for the comment header. Falls back to the raw
 * string if Date parsing returns Invalid Date — keeps the UI from rendering
 * "Invalid Date" on bad backend data.
 */
function formatCommentDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return format(d, 'MMM d, yyyy • h:mm a');
}

export function CommentsThread({ folder, uid }: CommentsThreadProps) {
  const queryClient = useQueryClient();
  const queryKey = ['comments', folder, uid] as const;

  const [newCommentText, setNewCommentText] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');

  const commentsQuery = useQuery({
    queryKey,
    queryFn: () => fetchComments(folder, uid),
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey });

  const createMut = useMutation({
    mutationFn: (content: string) => createComment(folder, uid, { content }),
    onSuccess: () => {
      setNewCommentText('');
      invalidate();
    },
  });

  const updateMut = useMutation({
    mutationFn: ({ id, content }: { id: string; content: string }) =>
      updateComment(id, { content }),
    onSuccess: () => {
      setEditingId(null);
      setEditingContent('');
      invalidate();
    },
  });

  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteComment(id),
    onSuccess: invalidate,
  });

  const handleSubmitNew = () => {
    const trimmed = newCommentText.trim();
    if (!trimmed || createMut.isPending) return;
    createMut.mutate(trimmed);
  };

  const handleStartEdit = (comment: EmailComment) => {
    setEditingId(comment.id);
    setEditingContent(comment.content);
  };

  const handleCancelEdit = () => {
    setEditingId(null);
    setEditingContent('');
  };

  const handleSaveEdit = () => {
    const trimmed = editingContent.trim();
    if (!trimmed || !editingId || updateMut.isPending) return;
    updateMut.mutate({ id: editingId, content: trimmed });
  };

  // Enter submits, Shift+Enter inserts a newline — matches the classic
  // SPA's CommentThread keyboard contract so muscle memory transfers.
  const handleKeyDown = (
    event: React.KeyboardEvent<HTMLTextAreaElement>,
    onSubmit: () => void,
  ) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  };

  const comments = commentsQuery.data ?? [];
  const count = comments.length;

  return (
    <section
      aria-label="Internal comments"
      data-testid="modern-comments-thread"
      className="mt-8 border-t border-zinc-200 dark:border-zinc-800 pt-6"
    >
      <header className="flex items-center gap-2 mb-4">
        <MessageSquare
          className="size-4 text-zinc-500 dark:text-zinc-400"
          aria-hidden="true"
        />
        <h3 className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
          Comments{count > 0 ? ` (${count})` : ''}
        </h3>
      </header>

      {commentsQuery.isLoading && (
        <div
          className="text-sm text-zinc-500 dark:text-zinc-400"
          data-testid="modern-comments-loading"
        >
          Loading comments…
        </div>
      )}

      {commentsQuery.isError && (
        <div
          role="alert"
          className="text-sm text-red-600 dark:text-red-400 mb-4"
        >
          Couldn't load comments: {String(commentsQuery.error)}
        </div>
      )}

      {!commentsQuery.isLoading && !commentsQuery.isError && count === 0 && (
        <div
          className="text-sm text-zinc-500 dark:text-zinc-400 mb-4"
          data-testid="modern-comments-empty"
        >
          No comments yet. Add an internal note below.
        </div>
      )}

      {count > 0 && (
        <ul className="space-y-3 mb-4" data-testid="modern-comments-list">
          {comments.map((comment) => {
            const isEditing = editingId === comment.id;
            return (
              <li
                key={comment.id}
                data-testid="modern-comment-item"
                data-comment-id={comment.id}
                className="rounded-lg border border-zinc-200 dark:border-zinc-800 bg-zinc-50 dark:bg-zinc-900 p-3"
              >
                <div className="flex items-center justify-between gap-2 mb-2">
                  <div className="min-w-0 flex-1">
                    <div className="text-sm font-medium truncate">
                      {comment.author_name}
                    </div>
                    <div className="text-xs text-zinc-500 dark:text-zinc-400">
                      {formatCommentDate(comment.created_at)}
                      {comment.updated_at !== comment.created_at
                        ? ' • edited'
                        : ''}
                    </div>
                  </div>
                  {!isEditing && (
                    <div className="flex items-center gap-1 shrink-0">
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        aria-label="Edit comment"
                        data-testid="modern-comment-edit"
                        disabled={updateMut.isPending || deleteMut.isPending}
                        onClick={() => handleStartEdit(comment)}
                      >
                        <Pencil className="size-4" aria-hidden="true" />
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        variant="ghost"
                        aria-label="Delete comment"
                        data-testid="modern-comment-delete"
                        disabled={deleteMut.isPending || updateMut.isPending}
                        onClick={() => deleteMut.mutate(comment.id)}
                      >
                        <Trash2 className="size-4 text-red-600 dark:text-red-400" aria-hidden="true" />
                      </Button>
                    </div>
                  )}
                </div>

                {isEditing ? (
                  <div className="space-y-2">
                    <Textarea
                      value={editingContent}
                      onChange={(e) => setEditingContent(e.target.value)}
                      onKeyDown={(e) => handleKeyDown(e, handleSaveEdit)}
                      data-testid="modern-comment-edit-input"
                      aria-label="Edit comment content"
                      className="min-h-20"
                    />
                    <div className="flex items-center gap-2">
                      <Button
                        type="button"
                        size="sm"
                        data-testid="modern-comment-save"
                        disabled={
                          updateMut.isPending || !editingContent.trim()
                        }
                        onClick={handleSaveEdit}
                      >
                        Save
                      </Button>
                      <Button
                        type="button"
                        size="sm"
                        variant="outline"
                        data-testid="modern-comment-cancel"
                        disabled={updateMut.isPending}
                        onClick={handleCancelEdit}
                      >
                        Cancel
                      </Button>
                    </div>
                  </div>
                ) : (
                  <p
                    className="text-sm whitespace-pre-wrap"
                    data-testid="modern-comment-content"
                  >
                    {comment.content}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}

      <div
        className="space-y-2"
        data-testid="modern-comments-new-form"
      >
        <Textarea
          placeholder="Add an internal comment…"
          value={newCommentText}
          onChange={(e) => setNewCommentText(e.target.value)}
          onKeyDown={(e) => handleKeyDown(e, handleSubmitNew)}
          aria-label="New comment content"
          data-testid="modern-comments-new-input"
          className="min-h-20"
        />
        {createMut.isError && (
          <div role="alert" className="text-sm text-red-600 dark:text-red-400">
            Failed to add comment: {String(createMut.error)}
          </div>
        )}
        <div className="flex items-center justify-end">
          <Button
            type="button"
            size="sm"
            data-testid="modern-comments-submit"
            disabled={createMut.isPending || !newCommentText.trim()}
            onClick={handleSubmitNew}
          >
            <Send className="size-4 mr-2" aria-hidden="true" />
            {createMut.isPending ? 'Adding…' : 'Add comment'}
          </Button>
        </div>
      </div>
    </section>
  );
}
