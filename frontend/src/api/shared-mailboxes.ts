import type {
  SharedMailboxView,
  SharedMailboxAclWithUser,
  SharedMailboxAcl,
  GrantAccessRequest,
} from '../types/shared-mailboxes';
import { apiClient } from './client';

export const sharedMailboxApi = {
  listAccessible: () =>
    apiClient.get<SharedMailboxView[]>('/shared-mailboxes'),

  listAcl: (mailboxId: string) =>
    apiClient.get<SharedMailboxAclWithUser[]>(`/shared-mailboxes/${mailboxId}/acl`),

  grantAccess: (mailboxId: string, data: GrantAccessRequest) =>
    apiClient.post<SharedMailboxAcl>(`/shared-mailboxes/${mailboxId}/acl`, data),

  revokeAccess: (mailboxId: string, userId: string) =>
    apiClient.delete(`/shared-mailboxes/${mailboxId}/acl/${userId}`),
};
