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

export async function listFilters(): Promise<SieveRule[]> {
  return apiClient.get('/api/filters');
}

export async function createFilter(data: CreateFilterRequest): Promise<SieveRule> {
  return apiClient.post('/api/filters', data);
}

export async function updateFilter(id: string, data: UpdateFilterRequest): Promise<SieveRule> {
  return apiClient.put(`/api/filters/${id}`, data);
}

export async function deleteFilter(id: string): Promise<void> {
  return apiClient.delete(`/api/filters/${id}`);
}

export async function reorderFilters(ruleIds: string[]): Promise<void> {
  return apiClient.post('/api/filters/reorder', ruleIds);
}
