// Added: Email comments API module for TMAIL-128 — internal comments on emails
import { apiClient } from './client';

// Added: Type representing an email comment from the backend
export interface EmailComment {
  id: string;
  mailbox_id: string;
  message_uid: number;
  folder: string;
  content: string;
  author_name: string;
  author_email: string;
  created_at: string;
  updated_at: string;
}

export interface CreateCommentRequest {
  content: string;
}

export interface UpdateCommentRequest {
  content: string;
}

// Added: Fetch all comments for a specific message
export async function fetchComments(folder: string, uid: number): Promise<EmailComment[]> {
  return apiClient.get<EmailComment[]>(`/folders/${encodeURIComponent(folder)}/messages/${uid}/comments`);
}

// Added: Create a new comment on a message
export async function createComment(folder: string, uid: number, data: CreateCommentRequest): Promise<EmailComment> {
  return apiClient.post<EmailComment>(`/folders/${encodeURIComponent(folder)}/messages/${uid}/comments`, data);
}

// Added: Update an existing comment
export async function updateComment(commentId: string, data: UpdateCommentRequest): Promise<EmailComment> {
  return apiClient.put<EmailComment>(`/comments/${commentId}`, data);
}

// Added: Delete a comment
export async function deleteComment(commentId: string): Promise<void> {
  await apiClient.delete(`/comments/${commentId}`);
}
