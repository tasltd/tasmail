// TMAIL-353: Modern UI LDAP/AD configuration API client. Mirrors
// frontend/src/api/ldap.ts. Backend offers explicit Test (bind check)
// and Sync (force-run) endpoints in addition to standard CRUD.
import { apiClient } from './client';

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

export const adminLdapApi = {
  list: () => apiClient.get<LdapConfiguration[]>('/admin/ldap'),
  create: (body: CreateLdapConfigRequest) =>
    apiClient.post<LdapConfiguration>('/admin/ldap', body),
  update: (id: string, body: UpdateLdapConfigRequest) =>
    apiClient.put<LdapConfiguration>(`/admin/ldap/${id}`, body),
  delete: (id: string) => apiClient.delete<void>(`/admin/ldap/${id}`),
  test: (id: string) => apiClient.post<void>(`/admin/ldap/${id}/test`),
  sync: (id: string) => apiClient.post<LdapSyncLog>(`/admin/ldap/${id}/sync`),
  logs: (id: string) => apiClient.get<LdapSyncLog[]>(`/admin/ldap/${id}/logs`),
};
