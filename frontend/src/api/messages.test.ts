import { describe, it, expect, vi, beforeEach } from 'vitest';
import { searchMessages, deleteMessage, moveMessage, flagMessage, saveDraft } from './messages';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('message API functions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('searchMessages', () => {
    it('calls GET /search with query parameter', async () => {
      const mockResponse = { messages: [], total: 0, query: 'test', folder: 'INBOX' };
      vi.mocked(apiClient.get).mockResolvedValue(mockResponse);

      const result = await searchMessages('test');
      expect(apiClient.get).toHaveBeenCalledWith('/search?q=test');
      expect(result).toEqual(mockResponse);
    });

    it('includes folder parameter when provided', async () => {
      const mockResponse = { messages: [], total: 0, query: 'test', folder: 'Sent' };
      vi.mocked(apiClient.get).mockResolvedValue(mockResponse);

      await searchMessages('test', 'Sent');
      expect(apiClient.get).toHaveBeenCalledWith('/search?q=test&folder=Sent');
    });

    it('encodes special characters in query', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ messages: [], total: 0, query: '', folder: '' });

      await searchMessages('hello world');
      expect(apiClient.get).toHaveBeenCalledWith('/search?q=hello+world');
    });

    // Added: Tests for advanced search params serialization (TMAIL-32)
    it('serializes AdvancedSearchParams with all fields', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ messages: [], total: 0, query: '', folder: '' });

      await searchMessages({
        query: 'report',
        folder: 'INBOX',
        from: 'alice@example.com',
        to: 'bob@example.com',
        subject: 'Monthly',
        dateFrom: '2026-01-01',
        dateTo: '2026-03-31',
        hasAttachment: true,
        isUnread: true,
        isStarred: false,
      });

      const calledUrl = vi.mocked(apiClient.get).mock.calls[0]?.[0] as string;
      const searchParams = new URLSearchParams(calledUrl.replace('/search?', ''));
      expect(searchParams.get('q')).toBe('report');
      expect(searchParams.get('folder')).toBe('INBOX');
      expect(searchParams.get('from')).toBe('alice@example.com');
      expect(searchParams.get('to')).toBe('bob@example.com');
      expect(searchParams.get('subject')).toBe('Monthly');
      expect(searchParams.get('date_from')).toBe('2026-01-01');
      expect(searchParams.get('date_to')).toBe('2026-03-31');
      expect(searchParams.get('has_attachment')).toBe('true');
      expect(searchParams.get('is_unread')).toBe('true');
      expect(searchParams.get('is_starred')).toBeNull();
    });

    it('omits empty advanced search fields from params', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ messages: [], total: 0, query: '', folder: '' });

      await searchMessages({
        query: 'test',
        from: 'sender@mail.com',
      });

      const calledUrl = vi.mocked(apiClient.get).mock.calls[0]?.[0] as string;
      const searchParams = new URLSearchParams(calledUrl.replace('/search?', ''));
      expect(searchParams.get('q')).toBe('test');
      expect(searchParams.get('from')).toBe('sender@mail.com');
      expect(searchParams.has('to')).toBe(false);
      expect(searchParams.has('subject')).toBe(false);
      expect(searchParams.has('date_from')).toBe(false);
      expect(searchParams.has('has_attachment')).toBe(false);
    });

    it('handles advanced params with only boolean filters', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ messages: [], total: 0, query: '', folder: '' });

      await searchMessages({
        query: '',
        hasAttachment: true,
        isStarred: true,
      });

      const calledUrl = vi.mocked(apiClient.get).mock.calls[0]?.[0] as string;
      const searchParams = new URLSearchParams(calledUrl.replace('/search?', ''));
      expect(searchParams.has('q')).toBe(false);
      expect(searchParams.get('has_attachment')).toBe('true');
      expect(searchParams.get('is_starred')).toBe('true');
    });
  });

  describe('deleteMessage', () => {
    it('calls DELETE on the correct endpoint', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteMessage('INBOX', 42);
      expect(apiClient.delete).toHaveBeenCalledWith('/folders/INBOX/messages/42');
    });

    it('encodes folder names with special characters', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);

      await deleteMessage('Sent Items', 10);
      expect(apiClient.delete).toHaveBeenCalledWith('/folders/Sent%20Items/messages/10');
    });
  });

  describe('moveMessage', () => {
    it('calls POST with to_folder in body', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await moveMessage('INBOX', 42, 'Archive');
      expect(apiClient.post).toHaveBeenCalledWith('/folders/INBOX/messages/42/move', {
        to_folder: 'Archive',
      });
    });
  });

  describe('flagMessage', () => {
    it('calls POST with flag and add=true', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await flagMessage('INBOX', 42, '\\Flagged', true);
      expect(apiClient.post).toHaveBeenCalledWith('/folders/INBOX/messages/42/flag', {
        flag: '\\Flagged',
        add: true,
      });
    });

    it('calls POST with flag and add=false to remove flag', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await flagMessage('INBOX', 42, '\\Seen', false);
      expect(apiClient.post).toHaveBeenCalledWith('/folders/INBOX/messages/42/flag', {
        flag: '\\Seen',
        add: false,
      });
    });
  });

  describe('saveDraft', () => {
    it('calls POST /drafts with draft data', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await saveDraft({
        to: ['user@example.com'],
        subject: 'Test draft',
        html_body: '<p>Hello</p>',
        text_body: 'Hello',
      });
      expect(apiClient.post).toHaveBeenCalledWith('/drafts', {
        to: ['user@example.com'],
        subject: 'Test draft',
        html_body: '<p>Hello</p>',
        text_body: 'Hello',
      });
    });

    it('sends draft without optional fields', async () => {
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await saveDraft({ to: ['a@b.com'], subject: '' });
      expect(apiClient.post).toHaveBeenCalledWith('/drafts', {
        to: ['a@b.com'],
        subject: '',
      });
    });
  });
});
