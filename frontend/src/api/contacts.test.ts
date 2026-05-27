import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchContacts, createContact, updateContact, deleteContact, importContactsCsv } from './contacts';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('contacts API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('fetchContacts', () => {
    it('calls GET /contacts without query', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchContacts();
      expect(apiClient.get).toHaveBeenCalledWith('/contacts');
    });

    it('calls GET /contacts with query parameter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchContacts('john');
      expect(apiClient.get).toHaveBeenCalledWith('/contacts?q=john');
    });

    it('encodes special characters in query', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await fetchContacts('john doe');
      expect(apiClient.get).toHaveBeenCalledWith('/contacts?q=john%20doe');
    });
  });

  describe('createContact', () => {
    it('calls POST /contacts with contact data', async () => {
      const contact = { email: 'john@example.com', display_name: 'John' };
      const mockResponse = { id: '1', ...contact, mailbox_id: '2', company: null, phone: null, notes: null, created_at: '', updated_at: '' };
      vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

      const result = await createContact(contact);
      expect(apiClient.post).toHaveBeenCalledWith('/contacts', contact);
      expect(result.email).toBe('john@example.com');
    });

    it('creates contact with minimal data', async () => {
      const contact = { email: 'min@test.com' };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '1', email: 'min@test.com' });

      await createContact(contact);
      expect(apiClient.post).toHaveBeenCalledWith('/contacts', { email: 'min@test.com' });
    });
  });

  describe('updateContact', () => {
    it('calls PUT /contacts/:id with update data', async () => {
      const update = { display_name: 'Updated Name' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', display_name: 'Updated Name' });

      await updateContact('abc', update);
      expect(apiClient.put).toHaveBeenCalledWith('/contacts/abc', update);
    });
  });

  describe('deleteContact', () => {
    it('calls DELETE /contacts/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteContact('abc');
      expect(apiClient.delete).toHaveBeenCalledWith('/contacts/abc');
    });
  });

  // Added: TMAIL-119 — CSV import client wrapper
  describe('importContactsCsv', () => {
    it('posts CSV text to /contacts/import-csv', async () => {
      const csv = 'email,name\nalice@example.com,Alice\n';
      vi.mocked(apiClient.post).mockResolvedValue({ imported: [], skipped: 0 });
      await importContactsCsv(csv);
      expect(apiClient.post).toHaveBeenCalledWith('/contacts/import-csv', { csv_text: csv });
    });

    it('returns imported contacts and skipped count', async () => {
      const mockContact = { id: '1', email: 'alice@example.com', display_name: 'Alice', mailbox_id: 'x', company: null, phone: null, notes: null, created_at: '', updated_at: '' };
      vi.mocked(apiClient.post).mockResolvedValue({ imported: [mockContact], skipped: 2 });
      const res = await importContactsCsv('email\nalice@example.com\n');
      expect(res.imported).toHaveLength(1);
      expect(res.skipped).toBe(2);
      expect(res.imported[0].email).toBe('alice@example.com');
    });
  });
});
