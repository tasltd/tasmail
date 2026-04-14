import { apiClient } from './client';

// Added: Email delegation types and API functions

export type DelegationType = 'send_as' | 'send_on_behalf';

export interface EmailDelegation {
  id: string;
  grantor_id: string;
  delegate_id: string;
  delegation_type: DelegationType;
  created_at: string;
}

export interface CreateDelegationRequest {
  grantor_id: string;
  delegate_id: string;
  delegation_type: DelegationType;
}

// Added: Grant a delegation from grantor to delegate
export async function grantDelegation(data: CreateDelegationRequest): Promise<EmailDelegation> {
  return apiClient.post('/api/delegation', data);
}

// Added: Revoke a delegation by id
export async function revokeDelegation(id: string): Promise<void> {
  return apiClient.delete(`/api/delegation/${id}`);
}

// Added: List delegations granted TO the current user
export async function listDelegations(): Promise<EmailDelegation[]> {
  return apiClient.get('/api/delegation');
}

// Added: List delegations the current user has granted to others
export async function listGrantedDelegations(): Promise<EmailDelegation[]> {
  return apiClient.get('/api/delegation/granted');
}
