import { describe, it, expect, vi, beforeEach } from 'vitest';
import { groupsApi } from './groups';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('groups API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('lists groups', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    await groupsApi.list();
    expect(apiClient.get).toHaveBeenCalledWith('/groups');
  });

  it('gets a single group', async () => {
    vi.mocked(apiClient.get).mockResolvedValue({ id: '1', name: 'Team' });
    const result = await groupsApi.get('1');
    expect(apiClient.get).toHaveBeenCalledWith('/groups/1');
    expect(result).toEqual({ id: '1', name: 'Team' });
  });

  it('creates a group', async () => {
    vi.mocked(apiClient.post).mockResolvedValue({ id: '1', name: 'New Group' });
    await groupsApi.create({ name: 'New Group', address: 'team@example.com', domain_id: 'd1', description: 'Test' });
    expect(apiClient.post).toHaveBeenCalledWith('/groups', { name: 'New Group', address: 'team@example.com', domain_id: 'd1', description: 'Test' });
  });

  it('updates a group', async () => {
    vi.mocked(apiClient.put).mockResolvedValue({ id: '1', name: 'Updated' });
    await groupsApi.update('1', { name: 'Updated' });
    expect(apiClient.put).toHaveBeenCalledWith('/groups/1', { name: 'Updated' });
  });

  it('deletes a group', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await groupsApi.delete('1');
    expect(apiClient.delete).toHaveBeenCalledWith('/groups/1');
  });

  it('lists members', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    await groupsApi.listMembers('1');
    expect(apiClient.get).toHaveBeenCalledWith('/groups/1/members');
  });

  it('adds a member', async () => {
    vi.mocked(apiClient.post).mockResolvedValue({ id: '2', address: 'user@test.com' });
    await groupsApi.addMember('1', { member_address: 'user@test.com' });
    expect(apiClient.post).toHaveBeenCalledWith('/groups/1/members', { member_address: 'user@test.com' });
  });

  it('removes a member with encoded address', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await groupsApi.removeMember('1', 'user@test.com');
    expect(apiClient.delete).toHaveBeenCalledWith('/groups/1/members/user%40test.com');
  });
});
