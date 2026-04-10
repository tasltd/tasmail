import { describe, it, expect, vi, beforeEach } from 'vitest';
import { sharedMailboxApi } from './shared-mailboxes';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('shared mailboxes API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists accessible shared mailboxes', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    await sharedMailboxApi.listAccessible();
    expect(apiClient.get).toHaveBeenCalledWith('/shared-mailboxes');
  });

  it('lists ACL for a mailbox', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    await sharedMailboxApi.listAcl('mailbox-123');
    expect(apiClient.get).toHaveBeenCalledWith('/shared-mailboxes/mailbox-123/acl');
  });

  it('grants access to a mailbox', async () => {
    const grantData = {
      granted_to: 'user-456',
      can_read: true,
      can_write: false,
      can_delete: false,
      can_admin: false,
    };
    vi.mocked(apiClient.post).mockResolvedValue({ id: '1', ...grantData });

    await sharedMailboxApi.grantAccess('mailbox-123', grantData);
    expect(apiClient.post).toHaveBeenCalledWith('/shared-mailboxes/mailbox-123/acl', grantData);
  });

  it('revokes access from a mailbox', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await sharedMailboxApi.revokeAccess('mailbox-123', 'user-456');
    expect(apiClient.delete).toHaveBeenCalledWith('/shared-mailboxes/mailbox-123/acl/user-456');
  });
});
