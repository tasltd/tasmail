// TMAIL-353: Modern UI SAML configuration API client. Mirrors
// frontend/src/api/saml.ts so the alt-UI Admin → SAML sub-tab can do
// CRUD against /api/admin/saml. The "Test" button calls
// GET /api/auth/saml/{id}/login — the backend builds the IdP redirect URL
// and any IdP-side validation surfaces here as a 4xx, so a 200 response
// (with a non-empty redirect_url) is the success signal.
import { apiClient } from './client';

export interface SamlConfiguration {
  id: string;
  name: string;
  entity_id: string;
  sso_url: string;
  slo_url: string | null;
  certificate: string;
  name_id_format: string;
  attribute_mapping: Record<string, string>;
  active: boolean;
  auto_create_users: boolean;
  created_at: string;
  updated_at: string;
}

export interface CreateSamlConfigRequest {
  name: string;
  entity_id: string;
  sso_url: string;
  slo_url?: string;
  certificate: string;
  name_id_format?: string;
  attribute_mapping?: Record<string, string>;
  auto_create_users?: boolean;
}

export interface UpdateSamlConfigRequest {
  name?: string;
  entity_id?: string;
  sso_url?: string;
  slo_url?: string;
  certificate?: string;
  name_id_format?: string;
  attribute_mapping?: Record<string, string>;
  active?: boolean;
  auto_create_users?: boolean;
}

export interface SamlLoginResponse {
  redirect_url: string;
}

export const adminSamlApi = {
  list: () => apiClient.get<SamlConfiguration[]>('/admin/saml'),
  create: (body: CreateSamlConfigRequest) =>
    apiClient.post<SamlConfiguration>('/admin/saml', body),
  update: (id: string, body: UpdateSamlConfigRequest) =>
    apiClient.put<SamlConfiguration>(`/admin/saml/${id}`, body),
  delete: (id: string) => apiClient.delete<void>(`/admin/saml/${id}`),
  // "Test" smoke check — fetches the IdP redirect URL the SP would send to.
  // Throws with a 400/404 if the config is inactive or unresolvable.
  test: (id: string) => apiClient.get<SamlLoginResponse>(`/auth/saml/${id}/login`),
};
