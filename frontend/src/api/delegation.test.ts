import { describe, it, expect, vi, beforeEach } from 'vitest';
import { grantDelegation, revokeDelegation, listDelegations, listGrantedDelegations } from './delegation';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('delegation API', () => {
  beforeEach(() => vi.clearAllMocks());

  it('grants a delegation', async () => {
    const mockDelegation = {
      id: 'd1',
      grantor_id: 'g1',
      delegate_id: 'u1',
      delegation_type: 'send_as',
      created_at: '2026-04-14T10:00:00Z',
    };
    vi.mocked(apiClient.post).mockResolvedValue(mockDelegation);
    const result = await grantDelegation({
      grantor_id: 'g1',
      delegate_id: 'u1',
      delegation_type: 'send_as',
    });
    expect(apiClient.post).toHaveBeenCalledWith('/api/delegation', expect.objectContaining({ delegation_type: 'send_as' }));
    expect(result.id).toBe('d1');
  });

  it('revokes a delegation', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await revokeDelegation('d1');
    expect(apiClient.delete).toHaveBeenCalledWith('/api/delegation/d1');
  });

  it('lists delegations for current user', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([]);
    const result = await listDelegations();
    expect(apiClient.get).toHaveBeenCalledWith('/api/delegation');
    expect(result).toHaveLength(0);
  });

  it('lists granted delegations', async () => {
    const mockDelegations = [
      { id: 'd1', grantor_id: 'g1', delegate_id: 'u1', delegation_type: 'send_on_behalf', created_at: '2026-04-14T10:00:00Z' },
    ];
    vi.mocked(apiClient.get).mockResolvedValue(mockDelegations);
    const result = await listGrantedDelegations();
    expect(apiClient.get).toHaveBeenCalledWith('/api/delegation/granted');
    expect(result).toHaveLength(1);
    expect(result[0].delegation_type).toBe('send_on_behalf');
  });
});
