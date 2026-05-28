// Added: LDAP/AD configuration API module for TMAIL-100
import { apiClient } from './client';

// Added: LDAP configuration interface matching backend LdapConfiguration struct
export interface LdapConfiguration {
  id: string;
  name: string;
  server_url: string;
  bind_dn: string;
  search_base: string;
  search_filter: string;
  email_attribute: string;
  name_attribute: string;
  group_filter: string | null;
  sync_interval_minutes: number;
  active: boolean;
  last_sync_at: string | null;
  last_sync_status: string | null;
  users_synced: number | null;
  created_at: string;
  updated_at: string;
}

// Added: Sync log interface matching backend LdapSyncLog struct
export interface LdapSyncLog {
  id: string;
  config_id: string;
  started_at: string;
  completed_at: string | null;
  users_created: number;
  users_updated: number;
  users_disabled: number;
  errors: Array<{ email?: string; error?: string }>;
  status: string;
}

// Added: Create request — required fields for new LDAP config
export interface CreateLdapConfigRequest {
  name: string;
  server_url: string;
  bind_dn: string;
  bind_password: string;
  search_base: string;
  search_filter?: string;
  email_attribute?: string;
  name_attribute?: string;
  group_filter?: string;
  sync_interval_minutes?: number;
}

// Added: Update request — all fields optional for partial updates
export interface UpdateLdapConfigRequest {
  name?: string;
  server_url?: string;
  bind_dn?: string;
  bind_password?: string;
  search_base?: string;
  search_filter?: string;
  email_attribute?: string;
  name_attribute?: string;
  group_filter?: string;
  sync_interval_minutes?: number;
  active?: boolean;
}

/**
 * PURPOSE: Fetch all LDAP/AD configurations (admin only)
 * EXTERNAL: GET /api/admin/ldap
 */
export async function listLdapConfigs(): Promise<LdapConfiguration[]> {
  return apiClient.get<LdapConfiguration[]>('/admin/ldap');
}

/**
 * PURPOSE: Create a new LDAP/AD configuration (admin only)
 * EXTERNAL: POST /api/admin/ldap
 */
export async function createLdapConfig(data: CreateLdapConfigRequest): Promise<LdapConfiguration> {
  return apiClient.post<LdapConfiguration>('/admin/ldap', data);
}

/**
 * PURPOSE: Update an existing LDAP/AD configuration (admin only)
 * EXTERNAL: PUT /api/admin/ldap/:id
 */
export async function updateLdapConfig(id: string, data: UpdateLdapConfigRequest): Promise<LdapConfiguration> {
  return apiClient.put<LdapConfiguration>(`/admin/ldap/${id}`, data);
}

/**
 * PURPOSE: Delete an LDAP/AD configuration (admin only)
 * EXTERNAL: DELETE /api/admin/ldap/:id
 */
export async function deleteLdapConfig(id: string): Promise<void> {
  return apiClient.delete(`/admin/ldap/${id}`);
}

/**
 * PURPOSE: Trigger manual LDAP sync for a configuration (admin only)
 * EXTERNAL: POST /api/admin/ldap/:id/sync
 */
export async function triggerLdapSync(id: string): Promise<LdapSyncLog> {
  return apiClient.post<LdapSyncLog>(`/admin/ldap/${id}/sync`);
}

/**
 * PURPOSE: Fetch sync history logs for a configuration (admin only)
 * EXTERNAL: GET /api/admin/ldap/:id/logs
 */
export async function listLdapSyncLogs(id: string): Promise<LdapSyncLog[]> {
  return apiClient.get<LdapSyncLog[]>(`/admin/ldap/${id}/logs`);
}

/**
 * PURPOSE: Verify the saved bind credentials can reach the LDAP server (admin only).
 * Returns void on success; throws with the LDAP error message on bind failure so
 * the UI can surface it to the admin.
 * EXTERNAL: POST /api/admin/ldap/:id/test
 */
export async function testLdapConnection(id: string): Promise<void> {
  await apiClient.post<void>(`/admin/ldap/${id}/test`);
}
