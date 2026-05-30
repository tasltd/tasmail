import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listFilters, createFilter, updateFilter, deleteFilter, reorderFilters } from './filters';
import type { SieveRule, CreateFilterRequest, RuleCondition, RuleAction } from './filters';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

const mockRule: SieveRule = {
  id: '550e8400-e29b-41d4-a716-446655440000',
  mailbox_id: '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
  name: 'Move newsletters',
  priority: 0,
  enabled: true,
  conditions: [{ field: 'from', operator: 'contains', value: 'newsletter' }],
  match_mode: 'all',
  actions: [{ action_type: 'move', target: 'Newsletters' }],
  stop_processing: true,
  created_at: '2026-04-10T00:00:00Z',
  updated_at: '2026-04-10T00:00:00Z',
};

describe('filters API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // Fix (TMAIL-319 follow-up to TMAIL-286): apiClient.request() already
  // prepends API_BASE_URL ("/api"), so the callers in filters.ts use
  // "/filters" (NOT "/api/filters"). These assertions were missed when
  // TMAIL-286 fixed the source but left the test asserting the bad URL —
  // every test in this file was failing pre-existing. Aligning the
  // assertions with the source is the smallest valid fix.
  it('lists filters', async () => {
    vi.mocked(apiClient.get).mockResolvedValue([mockRule]);
    const result = await listFilters();
    expect(apiClient.get).toHaveBeenCalledWith('/filters');
    expect(result).toHaveLength(1);
    expect(result[0].name).toBe('Move newsletters');
  });

  it('creates a filter', async () => {
    const request: CreateFilterRequest = {
      name: 'Spam filter',
      conditions: [{ field: 'from', operator: 'contains', value: 'spam' }],
      actions: [{ action_type: 'delete' }],
    };
    vi.mocked(apiClient.post).mockResolvedValue({ ...mockRule, name: 'Spam filter' });
    const result = await createFilter(request);
    expect(apiClient.post).toHaveBeenCalledWith('/filters', request);
    expect(result.name).toBe('Spam filter');
  });

  it('updates a filter', async () => {
    vi.mocked(apiClient.put).mockResolvedValue({ ...mockRule, name: 'Updated' });
    const result = await updateFilter('abc', { name: 'Updated' });
    expect(apiClient.put).toHaveBeenCalledWith('/filters/abc', { name: 'Updated' });
    expect(result.name).toBe('Updated');
  });

  it('deletes a filter', async () => {
    vi.mocked(apiClient.delete).mockResolvedValue(undefined);
    await deleteFilter('abc');
    expect(apiClient.delete).toHaveBeenCalledWith('/filters/abc');
  });

  it('reorders filters', async () => {
    vi.mocked(apiClient.post).mockResolvedValue(undefined);
    await reorderFilters(['id1', 'id2', 'id3']);
    expect(apiClient.post).toHaveBeenCalledWith('/filters/reorder', ['id1', 'id2', 'id3']);
  });
});

describe('filter types', () => {
  it('validates condition structure', () => {
    const condition: RuleCondition = {
      field: 'subject',
      operator: 'starts_with',
      value: '[URGENT]',
    };
    expect(condition.field).toBe('subject');
    expect(condition.operator).toBe('starts_with');
  });

  it('validates action structure with target', () => {
    const action: RuleAction = {
      action_type: 'forward',
      target: 'admin@company.com',
    };
    expect(action.action_type).toBe('forward');
    expect(action.target).toBe('admin@company.com');
  });

  it('validates action structure without target', () => {
    const action: RuleAction = {
      action_type: 'mark_read',
    };
    expect(action.action_type).toBe('mark_read');
    expect(action.target).toBeUndefined();
  });
});
