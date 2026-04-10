import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchContacts, createContact, updateContact, deleteContact } from './contacts';
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
});
