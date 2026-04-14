// Added: Contact groups API client tests for TMAIL-119
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listContactGroups,
  createContactGroup,
  updateContactGroup,
  deleteContactGroup,
  addContactToGroup,
  removeContactFromGroup,
  listContactsInGroup,
  importVcard,
  mergeContacts,
} from './contact-groups';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('contact-groups API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listContactGroups', () => {
    it('calls GET /contact-groups', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listContactGroups();
      expect(apiClient.get).toHaveBeenCalledWith('/contact-groups');
    });

    it('returns groups from response', async () => {
      const groups = [{ id: '1', name: 'Work', color: '#ff0000', user_id: 'u1', created_at: '' }];
      vi.mocked(apiClient.get).mockResolvedValue(groups);
      const result = await listContactGroups();
      expect(result).toEqual(groups);
    });
  });

  describe('createContactGroup', () => {
    it('calls POST /contact-groups with data', async () => {
      const data = { name: 'Friends', color: '#00ff00' };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '1', ...data });
      await createContactGroup(data);
      expect(apiClient.post).toHaveBeenCalledWith('/contact-groups', data);
    });

    it('creates group with name only', async () => {
      const data = { name: 'Work' };
      vi.mocked(apiClient.post).mockResolvedValue({ id: '2', name: 'Work' });
      await createContactGroup(data);
      expect(apiClient.post).toHaveBeenCalledWith('/contact-groups', { name: 'Work' });
    });
  });

  describe('updateContactGroup', () => {
    it('calls PUT /contact-groups/:id with data', async () => {
      const data = { name: 'Renamed' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'g1', name: 'Renamed' });
      await updateContactGroup('g1', data);
      expect(apiClient.put).toHaveBeenCalledWith('/contact-groups/g1', data);
    });
  });

  describe('deleteContactGroup', () => {
    it('calls DELETE /contact-groups/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteContactGroup('g1');
      expect(apiClient.delete).toHaveBeenCalledWith('/contact-groups/g1');
    });
  });

  describe('addContactToGroup', () => {
    it('calls POST /contact-groups/:id/members with contact_id', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({ contact_group_id: 'g1', contact_id: 'c1' });
      await addContactToGroup('g1', 'c1');
      expect(apiClient.post).toHaveBeenCalledWith('/contact-groups/g1/members', { contact_id: 'c1' });
    });
  });

  describe('removeContactFromGroup', () => {
    it('calls DELETE /contact-groups/:id/members/:contact_id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await removeContactFromGroup('g1', 'c1');
      expect(apiClient.delete).toHaveBeenCalledWith('/contact-groups/g1/members/c1');
    });
  });

  describe('listContactsInGroup', () => {
    it('calls GET /contact-groups/:id/contacts', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listContactsInGroup('g1');
      expect(apiClient.get).toHaveBeenCalledWith('/contact-groups/g1/contacts');
    });

    it('returns contacts from response', async () => {
      const contacts = [{ id: 'c1', email: 'alice@test.com' }];
      vi.mocked(apiClient.get).mockResolvedValue(contacts);
      const result = await listContactsInGroup('g1');
      expect(result).toEqual(contacts);
    });
  });

  describe('importVcard', () => {
    it('calls POST /contacts/import-vcard with vcard_text', async () => {
      const vcardText = 'BEGIN:VCARD\nFN:Test\nEMAIL:test@test.com\nEND:VCARD';
      vi.mocked(apiClient.post).mockResolvedValue([]);
      await importVcard(vcardText);
      expect(apiClient.post).toHaveBeenCalledWith('/contacts/import-vcard', { vcard_text: vcardText });
    });
  });

  describe('mergeContacts', () => {
    it('calls POST /contacts/merge with contact_ids', async () => {
      const ids = ['c1', 'c2', 'c3'];
      vi.mocked(apiClient.post).mockResolvedValue({ id: 'c1', email: 'primary@test.com' });
      await mergeContacts(ids);
      expect(apiClient.post).toHaveBeenCalledWith('/contacts/merge', { contact_ids: ids });
    });

    it('returns the primary contact', async () => {
      const primary = { id: 'c1', email: 'primary@test.com' };
      vi.mocked(apiClient.post).mockResolvedValue(primary);
      const result = await mergeContacts(['c1', 'c2']);
      expect(result).toEqual(primary);
    });
  });
});
