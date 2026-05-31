// TMAIL-353: Modern UI OIDC provider API client (admin surface). The
// /auth/oidc/* public endpoints used by the login page are already
// exposed via api/oidc.ts — this module covers the admin-only CRUD +
// the "Test" button that hits GET /api/auth/oidc/{id}/authorize.
import { apiClient } from './client';

export interface OidcProvider {
  id: string;
  name: string;
  issuer_url: string;
  client_id: string;
  scopes: string;
  redirect_uri: string;
  auto_create_users: boolean;
  default_role: string;
  active: boolean;
  icon_url: string | null;
  button_label: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateOidcProviderRequest {
  name: string;
  issuer_url: string;
  client_id: string;
  client_secret: string;
  redirect_uri: string;
  scopes?: string;
  auto_create_users?: boolean;
  default_role?: string;
  icon_url?: string;
  button_label?: string;
}

export interface UpdateOidcProviderRequest {
  name?: string;
  issuer_url?: string;
  client_id?: string;
  client_secret?: string;
  scopes?: string;
  redirect_uri?: string;
  auto_create_users?: boolean;
  default_role?: string;
  active?: boolean;
  icon_url?: string;
  button_label?: string;
}

export interface OidcAuthorizeResponse {
  authorize_url: string;
  state: string;
}

export const adminOidcApi = {
  list: () => apiClient.get<OidcProvider[]>('/admin/oidc'),
  create: (body: CreateOidcProviderRequest) =>
    apiClient.post<OidcProvider>('/admin/oidc', body),
  update: (id: string, body: UpdateOidcProviderRequest) =>
    apiClient.put<OidcProvider>(`/admin/oidc/${id}`, body),
  delete: (id: string) => apiClient.delete<void>(`/admin/oidc/${id}`),
  // "Test" smoke check — fetches a real authorize URL from the OIDC
  // provider's discovery doc; 4xx if the issuer URL is unreachable or
  // discovery fails.
  test: (id: string) => apiClient.get<OidcAuthorizeResponse>(`/auth/oidc/${id}/authorize`),
};
