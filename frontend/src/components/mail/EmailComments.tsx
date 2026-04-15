// Added: Internal email comments component for TMAIL-128
// PURPOSE: Displays and manages internal comments attached to a specific email message
// CONSTRAINTS: Compact inline display — this is a mail sub-component, not a full-page settings view
// EXTERNAL: Uses TanStack Query for data fetching, comments API for CRUD

import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { MessageSquare, Send, Pencil, Trash2, X, Check } from 'lucide-react';
import { fetchComments, createComment, updateComment, deleteComment } from '../../api/comments';
import type { EmailComment } from '../../api/comments';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Props for the EmailComments component
interface EmailCommentsProps {
  folder: string;
  uid: number;
}

export function EmailComments({ folder, uid }: EmailCommentsProps) {
  const queryClient = useQueryClient();
  const [newComment, setNewComment] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');

  // Added: Fetch all comments for this message
  const { data: comments, isLoading } = useQuery({
    queryKey: ['comments', folder, uid],
    queryFn: () => fetchComments(folder, uid),
  });

  // Added: Create a new comment
  const createMut = useMutation({
    mutationFn: (content: string) => createComment(folder, uid, { content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] });
      setNewComment('');
    },
  });

  // Added: Update an existing comment
  const updateMut = useMutation({
    mutationFn: ({ commentId, content }: { commentId: string; content: string }) =>
      updateComment(commentId, { content }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] });
      setEditingId(null);
      setEditContent('');
    },
  });

  // Added: Delete a comment
  const deleteMut = useMutation({
    mutationFn: deleteComment,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['comments', folder, uid] }),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!newComment.trim()) return;
    createMut.mutate(newComment.trim());
  };

  const handleEditSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!editingId || !editContent.trim()) return;
    updateMut.mutate({ commentId: editingId, content: editContent.trim() });
  };

  const startEditing = (comment: EmailComment) => {
    setEditingId(comment.id);
    setEditContent(comment.content);
  };

  return (
    <div
      className="email-comments"
      style={{
        borderTop: '1px solid var(--color-border)',
        padding: '12px',
        fontSize: '13px',
      }}
    >
      {/* Added: Section header */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '8px' }}>
        <MessageSquare size={14} style={{ color: 'var(--color-text-secondary)' }} />
        <strong>Comments</strong>
        {comments && comments.length > 0 && (
          <span
            style={{
              fontSize: '11px',
              padding: '0 6px',
              borderRadius: '10px',
              background: 'var(--color-bg-secondary)',
              color: 'var(--color-text-secondary)',
            }}
          >
            {comments.length}
          </span>
        )}
      </div>

      {/* Added: Loading state */}
      {isLoading && <LoadingSkeleton rows={2} />}

      {/* Added: Empty state */}
      {!isLoading && (!comments || comments.length === 0) && (
        <p style={{ color: 'var(--color-text-secondary)', margin: '4px 0 12px', fontSize: '12px' }}>
          No comments yet. Be the first to add one.
        </p>
      )}

      {/* Added: Comment list */}
      {comments?.map((comment: EmailComment) => (
        <div
          key={comment.id}
          style={{
            padding: '8px',
            marginBottom: '6px',
            borderRadius: '6px',
            background: 'var(--color-bg-secondary)',
          }}
          data-testid={`comment-${comment.id}`}
        >
          {editingId === comment.id ? (
            // Added: Inline edit form
            <form onSubmit={handleEditSubmit} style={{ display: 'flex', gap: '6px', alignItems: 'center' }}>
              <input
                value={editContent}
                onChange={(e) => setEditContent(e.target.value)}
                style={{ flex: 1, fontSize: '12px' }}
                autoFocus
              />
              <button
                type="submit"
                className="btn btn--icon"
                disabled={!editContent.trim() || updateMut.isPending}
                title="Save"
              >
                <Check size={14} />
              </button>
              <button
                type="button"
                className="btn btn--icon"
                onClick={() => setEditingId(null)}
                title="Cancel"
              >
                <X size={14} />
              </button>
            </form>
          ) : (
            <>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start' }}>
                <div>
                  <strong style={{ fontSize: '12px' }}>{comment.author_name}</strong>
                  <span style={{ color: 'var(--color-text-secondary)', fontSize: '11px', marginLeft: '6px' }}>
                    {new Date(comment.created_at).toLocaleString()}
                  </span>
                </div>
                <div style={{ display: 'flex', gap: '4px' }}>
                  <button
                    className="btn btn--icon"
                    onClick={() => startEditing(comment)}
                    title="Edit"
                    style={{ padding: '2px' }}
                  >
                    <Pencil size={12} />
                  </button>
                  <button
                    className="btn btn--icon"
                    onClick={() => deleteMut.mutate(comment.id)}
                    title="Delete"
                    style={{ padding: '2px' }}
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>
              <div style={{ marginTop: '4px', fontSize: '12px', lineHeight: '1.4' }}>
                {comment.content}
              </div>
            </>
          )}
        </div>
      ))}

      {/* Added: New comment form */}
      <form onSubmit={handleSubmit} style={{ display: 'flex', gap: '6px', marginTop: '8px' }}>
        <input
          value={newComment}
          onChange={(e) => setNewComment(e.target.value)}
          placeholder="Add a comment..."
          style={{ flex: 1, fontSize: '12px' }}
        />
        <button
          type="submit"
          className="btn btn--icon btn--primary"
          disabled={!newComment.trim() || createMut.isPending}
          title="Send"
        >
          <Send size={14} />
        </button>
      </form>
    </div>
  );
}
