import { apiClient } from './client';

export interface RuleCondition {
  field: 'from' | 'to' | 'cc' | 'subject' | 'body' | 'header' | 'size';
  operator: 'contains' | 'not_contains' | 'equals' | 'starts_with' | 'ends_with' | 'matches_regex' | 'greater_than' | 'less_than';
  value: string;
}

export interface RuleAction {
  action_type: 'move' | 'copy' | 'delete' | 'mark_read' | 'mark_flagged' | 'forward' | 'reject' | 'add_label' | 'stop';
  target?: string;
}

export interface SieveRule {
  id: string;
  mailbox_id: string;
  name: string;
  priority: number;
  enabled: boolean;
  conditions: RuleCondition[];
  match_mode: 'all' | 'any';
  actions: RuleAction[];
  stop_processing: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateFilterRequest {
  name: string;
  priority?: number;
  enabled?: boolean;
  conditions: RuleCondition[];
  match_mode?: 'all' | 'any';
  actions: RuleAction[];
  stop_processing?: boolean;
}

export interface UpdateFilterRequest {
  name?: string;
  priority?: number;
  enabled?: boolean;
  conditions?: RuleCondition[];
  match_mode?: 'all' | 'any';
  actions?: RuleAction[];
  stop_processing?: boolean;
}

// Fix (TMAIL-286): apiClient.request() already prepends API_BASE_URL ("/api"),
// so paths must be relative — every other api module here uses '/<resource>'.
// The previous '/api/filters' resolved to '/api/api/filters' → 404 on every
// FilterManager CRUD call, leaving the whole Filters surface non-functional.
export async function listFilters(): Promise<SieveRule[]> {
  return apiClient.get('/filters');
}

export async function createFilter(data: CreateFilterRequest): Promise<SieveRule> {
  return apiClient.post('/filters', data);
}

export async function updateFilter(id: string, data: UpdateFilterRequest): Promise<SieveRule> {
  return apiClient.put(`/filters/${id}`, data);
}

export async function deleteFilter(id: string): Promise<void> {
  return apiClient.delete(`/filters/${id}`);
}

export async function reorderFilters(ruleIds: string[]): Promise<void> {
  return apiClient.post('/filters/reorder', ruleIds);
}

// Added (TMAIL-286): Test-match — evaluate the saved rule against a synthetic
// sample message so users can sanity-check their filter before it goes live.
export interface SampleMessage {
  from?: string;
  to?: string;
  cc?: string;
  subject?: string;
  body?: string;
  size?: number;
}

export interface ConditionMatch {
  field: string;
  operator: string;
  value: string;
  matched: boolean;
}

export interface RuleMatchResult {
  matched: boolean;
  match_mode: 'all' | 'any';
  condition_results: ConditionMatch[];
}

export async function testFilter(id: string, sample: SampleMessage): Promise<RuleMatchResult> {
  return apiClient.post<RuleMatchResult>(`/filters/${id}/test`, sample);
}
