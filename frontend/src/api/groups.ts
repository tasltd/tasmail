import type { DistributionGroup, GroupMember, CreateGroupRequest, UpdateGroupRequest, AddMemberRequest } from '../types/groups';
import { apiClient } from './client';

export const groupsApi = {
  list: () => apiClient.get<DistributionGroup[]>('/groups'),

  get: (id: string) => apiClient.get<DistributionGroup>(`/groups/${id}`),

  create: (data: CreateGroupRequest) =>
    apiClient.post<DistributionGroup>('/groups', data),

  update: (id: string, data: UpdateGroupRequest) =>
    apiClient.put<DistributionGroup>(`/groups/${id}`, data),

  delete: (id: string) => apiClient.delete(`/groups/${id}`),

  listMembers: (groupId: string) =>
    apiClient.get<GroupMember[]>(`/groups/${groupId}/members`),

  addMember: (groupId: string, data: AddMemberRequest) =>
    apiClient.post<GroupMember>(`/groups/${groupId}/members`, data),

  removeMember: (groupId: string, address: string) =>
    apiClient.delete(`/groups/${groupId}/members/${encodeURIComponent(address)}`),
};
