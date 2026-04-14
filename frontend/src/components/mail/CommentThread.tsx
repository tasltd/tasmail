// Added: CommentThread component for TMAIL-128 — internal comments on emails
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { MessageSquare, Send, Pencil, Trash2, ChevronDown, ChevronRight } from 'lucide-react';
import { fetchComments, createComment, updateComment, deleteComment } from '../../api/comments';
import type { EmailComment } from '../../api/comments';
import { formatFullDate } from '../../utils/date';

/**
 * PURPOSE: Display and manage internal comments/notes on an email message
 * CONSTRAINTS: Comments are visible only to the organization, not sent externally
 * EXTERNAL: Uses TanStack Query for data fetching, comments API for CRUD
 */
interface CommentThreadProps {
  folder: string;
  uid: number;
}

export function CommentThread({ folder, uid }: CommentThreadProps) {
  const queryClient = useQueryClient();
  const [isExpanded, setIsExpanded] = useState(false);
  const [newCommentText, setNewCommentText] = useState('');
  const [editingCommentId, setEditingCommentId] = useState<string | null>(null);
  const [editingContent, setEditingContent] = useState('');

  // Added: Query to fetch comments for this message
  const { data: comments = [], isLoading } = useQuery({
    queryKey: ['comments', folder, uid],
    queryFn: () => fetchComments(folder, uid),
    // NOTE: Only fetch when the thread is expanded to reduce unnecessary requests
    enabled: isExpanded,
  });

  // Added: Mutation to create a new comment
  const createMut = useMutation({
    mutationFn: (content: string) => createComment(folder, uid, { content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] });
      setNewCommentText('');
    },
  });

  // Added: Mutation to update an existing comment
  const updateMut = useMutation({
    mutationFn: ({ commentId, content }: { commentId: string; content: string }) =>
      updateComment(commentId, { content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] });
      setEditingCommentId(null);
      setEditingContent('');
    },
  });

  // Added: Mutation to delete a comment
  const deleteMut = useMutation({
    mutationFn: (commentId: string) => deleteComment(commentId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] });
    },
  });

  const handleSubmitNewComment = () => {
    const trimmedContent = newCommentText.trim();
    if (!trimmedContent) return;
    createMut.mutate(trimmedContent);
  };

  const handleStartEdit = (comment: EmailComment) => {
    setEditingCommentId(comment.id);
    setEditingContent(comment.content);
  };

  const handleSaveEdit = () => {
    const trimmedContent = editingContent.trim();
    if (!trimmedContent || !editingCommentId) return;
    updateMut.mutate({ commentId: editingCommentId, content: trimmedContent });
  };

  const handleCancelEdit = () => {
    setEditingCommentId(null);
    setEditingContent('');
  };

  // Added: Handle Enter key to submit comment (Shift+Enter for newline)
  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>, onSubmit: () => void) => {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      onSubmit();
    }
  };

  const commentCount = comments.length;

  return (
    <div className="comment-thread" data-testid="comment-thread">
      {/* Added: Collapsible header showing comment count */}
      <button
        className="comment-thread__toggle"
        onClick={() => setIsExpanded(!isExpanded)}
        data-testid="comment-toggle"
      >
        {isExpanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        <MessageSquare size={16} />
        <span>
          {commentCount > 0
            ? `${commentCount} comment${commentCount !== 1 ? 's' : ''}`
            : 'Comments'}
        </span>
      </button>

      {isExpanded && (
        <div className="comment-thread__body" data-testid="comment-body">
          {/* Added: Loading state */}
          {isLoading && <div className="comment-thread__loading">Loading comments...</div>}

          {/* Added: Comment list */}
          {!isLoading && comments.length === 0 && (
            <div className="comment-thread__empty" data-testid="comment-empty">
              No comments yet. Add an internal note below.
            </div>
          )}

          {comments.map((comment) => (
            <div key={comment.id} className="comment-thread__item" data-testid="comment-item">
              <div className="comment-thread__item-header">
                <strong className="comment-thread__author">{comment.author_name}</strong>
                <span className="comment-thread__date">{formatFullDate(comment.created_at)}</span>
                {/* Added: Edit and delete buttons */}
                <div className="comment-thread__actions">
                  <button
                    className="btn btn--icon btn--small"
                    onClick={() => handleStartEdit(comment)}
                    title="Edit comment"
                    data-testid="comment-edit-btn"
                  >
                    <Pencil size={14} />
                  </button>
                  <button
                    className="btn btn--icon btn--small btn--danger"
                    onClick={() => deleteMut.mutate(comment.id)}
                    title="Delete comment"
                    data-testid="comment-delete-btn"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </div>

              {/* Added: Show edit form or content based on editing state */}
              {editingCommentId === comment.id ? (
                <div className="comment-thread__edit-form">
                  <textarea
                    className="comment-thread__textarea"
                    value={editingContent}
                    onChange={(e) => setEditingContent(e.target.value)}
                    onKeyDown={(e) => handleKeyDown(e, handleSaveEdit)}
                    data-testid="comment-edit-input"
                  />
                  <div className="comment-thread__edit-actions">
                    <button
                      className="btn btn--small btn--primary"
                      onClick={handleSaveEdit}
                      disabled={updateMut.isPending || !editingContent.trim()}
                      data-testid="comment-save-btn"
                    >
                      Save
                    </button>
                    <button
                      className="btn btn--small"
                      onClick={handleCancelEdit}
                      data-testid="comment-cancel-btn"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              ) : (
                <p className="comment-thread__content" data-testid="comment-content">
                  {comment.content}
                </p>
              )}
            </div>
          ))}

          {/* Added: New comment input */}
          <div className="comment-thread__new" data-testid="comment-new-form">
            <textarea
              className="comment-thread__textarea"
              placeholder="Add an internal comment..."
              value={newCommentText}
              onChange={(e) => setNewCommentText(e.target.value)}
              onKeyDown={(e) => handleKeyDown(e, handleSubmitNewComment)}
              data-testid="comment-new-input"
            />
            <button
              className="btn btn--primary btn--small comment-thread__submit"
              onClick={handleSubmitNewComment}
              disabled={createMut.isPending || !newCommentText.trim()}
              title="Add comment"
              data-testid="comment-submit-btn"
            >
              <Send size={14} />
              <span>Add</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
