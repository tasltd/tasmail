// TMAIL-348: Email comments API client for the Modern UI EmailReader.
//
// Mirrors `frontend/src/api/comments.ts` (the classic SPA's client) so the
// two surfaces hit the exact same backend contract — see
// `backend/src/handlers/comments.rs` (TMAIL-128) for the route handlers wired
// in `router.rs:161-167`.
//
// Endpoints consumed:
//   GET    /api/folders/{folder}/messages/{uid}/comments
//   POST   /api/folders/{folder}/messages/{uid}/comments
//   PUT    /api/comments/{id}
//   DELETE /api/comments/{id}
//
// Comments are mailbox-scoped server-side (PostgreSQL RLS + explicit
// mailbox_id check in the update/delete queries), so every comment the
// signed-in user can read IS theirs by definition — there's no cross-user
// authorship to gate. That's why this client doesn't carry a "current user"
// identity: the backend already enforces ownership.
import { apiClient } from './client';

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

/** GET /api/folders/{folder}/messages/{uid}/comments */
export function fetchComments(folder: string, uid: number): Promise<EmailComment[]> {
  return apiClient.get<EmailComment[]>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}/comments`,
  );
}

/** POST /api/folders/{folder}/messages/{uid}/comments */
export function createComment(
  folder: string,
  uid: number,
  data: CreateCommentRequest,
): Promise<EmailComment> {
  return apiClient.post<EmailComment>(
    `/folders/${encodeURIComponent(folder)}/messages/${uid}/comments`,
    data,
  );
}

/** PUT /api/comments/{id} */
export function updateComment(
  commentId: string,
  data: UpdateCommentRequest,
): Promise<EmailComment> {
  return apiClient.put<EmailComment>(`/comments/${commentId}`, data);
}

/** DELETE /api/comments/{id} */
export async function deleteComment(commentId: string): Promise<void> {
  await apiClient.delete<void>(`/comments/${commentId}`);
}
