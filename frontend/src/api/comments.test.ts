// Added: Tests for email comments API module (TMAIL-128)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchComments, createComment, updateComment, deleteComment } from './comments';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('comments API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchComments', () => {
    it('calls GET /folders/{folder}/messages/{uid}/comments', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await fetchComments('INBOX', 42);
      expect(apiClient.get).toHaveBeenCalledWith('/folders/INBOX/messages/42/comments');
      expect(result).toEqual([]);
    });

    it('encodes folder name with special characters', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchComments('Sent Items', 7);
      expect(apiClient.get).toHaveBeenCalledWith('/folders/Sent%20Items/messages/7/comments');
    });
  });

  describe('createComment', () => {
    it('calls POST with comment content', async () => {
      const commentData = { content: 'Need to follow up on this' };
      const mockResponse = { id: 'abc-123', ...commentData, author_name: 'Test User' };
      vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

      const result = await createComment('INBOX', 42, commentData);
      expect(apiClient.post).toHaveBeenCalledWith(
        '/folders/INBOX/messages/42/comments',
        commentData
      );
      expect(result.content).toBe('Need to follow up on this');
    });
  });

  describe('updateComment', () => {
    it('calls PUT /comments/{id} with updated content', async () => {
      const updateData = { content: 'Updated note' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc-123', content: 'Updated note' });

      const result = await updateComment('abc-123', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/comments/abc-123', updateData);
      expect(result.content).toBe('Updated note');
    });
  });

  describe('deleteComment', () => {
    it('calls DELETE /comments/{id}', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteComment('abc-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/comments/abc-123');
    });
  });

  describe('fetchComments returns multiple comments', () => {
    it('returns array of comments ordered by creation', async () => {
      const mockComments = [
        { id: '1', content: 'First comment', created_at: '2026-04-10T10:00:00Z' },
        { id: '2', content: 'Second comment', created_at: '2026-04-10T11:00:00Z' },
      ];
      vi.mocked(apiClient.get).mockResolvedValue(mockComments);

      const result = await fetchComments('INBOX', 5);
      expect(result).toHaveLength(2);
      expect(result[0].content).toBe('First comment');
      expect(result[1].content).toBe('Second comment');
    });
  });
});
