// Added: CalDAV/CardDAV configuration API client for TMAIL-117

import { apiClient } from './client';

// Added: DAV type enum matching backend
export type DavType = 'caldav' | 'carddav' | 'both';

export interface DavConfiguration {
  id: string;
  user_id: string;
  name: string;
  server_url: string;
  username: string;
  password_masked: string;
  dav_type: string;
  sync_interval_minutes: number;
  last_sync_at: string | null;
  sync_status: string | null;
  sync_error: string | null;
  enabled: boolean;
  created_at: string | null;
  updated_at: string | null;
}

export interface CreateDavConfigRequest {
  name: string;
  server_url: string;
  username: string;
  password: string;
  dav_type: DavType;
  sync_interval_minutes?: number;
  enabled?: boolean;
}

export interface UpdateDavConfigRequest {
  name?: string;
  server_url?: string;
  username?: string;
  password?: string;
  dav_type?: DavType;
  sync_interval_minutes?: number;
  enabled?: boolean;
}

export interface DavTestResult {
  success: boolean;
  message: string;
  latency_ms: number;
}

// PURPOSE: List all DAV configs for the current user (passwords masked)
export async function listDavConfigs(): Promise<DavConfiguration[]> {
  return apiClient.get<DavConfiguration[]>('/dav/configs');
}

// PURPOSE: Create a new DAV configuration
export async function createDavConfig(data: CreateDavConfigRequest): Promise<DavConfiguration> {
  return apiClient.post<DavConfiguration>('/dav/configs', data);
}

// PURPOSE: Get a single DAV configuration by ID
export async function getDavConfig(id: string): Promise<DavConfiguration> {
  return apiClient.get<DavConfiguration>(`/dav/configs/${id}`);
}

// PURPOSE: Update an existing DAV configuration
export async function updateDavConfig(id: string, data: UpdateDavConfigRequest): Promise<DavConfiguration> {
  return apiClient.put<DavConfiguration>(`/dav/configs/${id}`, data);
}

// PURPOSE: Delete a DAV configuration
export async function deleteDavConfig(id: string): Promise<void> {
  await apiClient.delete(`/dav/configs/${id}`);
}

// PURPOSE: Trigger a manual sync for a DAV configuration
export async function syncDavConfig(id: string): Promise<DavConfiguration> {
  return apiClient.post<DavConfiguration>(`/dav/configs/${id}/sync`, {});
}

// PURPOSE: Test connection to a CalDAV/CardDAV server
export async function testDavConfig(id: string): Promise<DavTestResult> {
  return apiClient.post<DavTestResult>(`/dav/configs/${id}/test`, {});
}
