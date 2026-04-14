// Added: Plugin API client for plugin/extension architecture (TMAIL-132)

import { apiClient } from './client';

// Added: Plugin type enum matching backend
export type PluginType = 'webhook' | 'script' | 'filter';

// Added: Plugin hook enum matching backend
export type PluginHook =
  | 'on_receive'
  | 'on_send'
  | 'on_delete'
  | 'on_move'
  | 'on_flag'
  | 'on_read';

export interface Plugin {
  id: string;
  user_id: string | null;
  name: string;
  description: string | null;
  plugin_type: string;
  config: Record<string, unknown>;
  hooks: string[];
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface PluginExecution {
  id: string;
  plugin_id: string;
  event: string;
  status: 'success' | 'error' | 'timeout';
  duration_ms: number | null;
  error_message: string | null;
  executed_at: string;
}

export interface CreatePluginRequest {
  name: string;
  description?: string;
  plugin_type: PluginType;
  config?: Record<string, unknown>;
  hooks: PluginHook[];
  enabled?: boolean;
}

export interface UpdatePluginRequest {
  name?: string;
  description?: string;
  plugin_type?: PluginType;
  config?: Record<string, unknown>;
  hooks?: PluginHook[];
  enabled?: boolean;
}

export interface TestPluginResponse {
  success: boolean;
  duration_ms: number;
  error: string | null;
}

// PURPOSE: List all plugins for the current user (plus system-wide)
export async function listPlugins(): Promise<Plugin[]> {
  return apiClient.get<Plugin[]>('/plugins');
}

// PURPOSE: Create a new plugin
export async function createPlugin(data: CreatePluginRequest): Promise<Plugin> {
  return apiClient.post<Plugin>('/plugins', data);
}

// PURPOSE: Get a single plugin by ID
export async function getPlugin(id: string): Promise<Plugin> {
  return apiClient.get<Plugin>(`/plugins/${id}`);
}

// PURPOSE: Update an existing plugin
export async function updatePlugin(id: string, data: UpdatePluginRequest): Promise<Plugin> {
  return apiClient.put<Plugin>(`/plugins/${id}`, data);
}

// PURPOSE: Delete a plugin and all its execution records
export async function deletePlugin(id: string): Promise<void> {
  await apiClient.delete(`/plugins/${id}`);
}

// PURPOSE: List recent execution log entries for a plugin
export async function listExecutions(pluginId: string): Promise<PluginExecution[]> {
  return apiClient.get<PluginExecution[]>(`/plugins/${pluginId}/executions`);
}

// PURPOSE: Test-fire a plugin with dummy context
export async function testPlugin(id: string): Promise<TestPluginResponse> {
  return apiClient.post<TestPluginResponse>(`/plugins/${id}/test`, {});
}
