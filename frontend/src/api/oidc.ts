// Added: OIDC provider API module for TMAIL-99
import { apiClient } from './client';

// Added: OIDC provider interface matching backend OidcProvider struct
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

// Added: Public-facing OIDC provider info for login page display
export interface OidcLoginProvider {
  id: string;
  name: string;
  icon_url: string | null;
  button_label: string | null;
}

// Added: Create request — required fields for new OIDC provider
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

// Added: Update request — all fields optional for partial updates
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

// Added: Authorization URL response from the authorize endpoint
export interface OidcAuthorizeResponse {
  authorize_url: string;
  state: string;
}

/**
 * PURPOSE: Fetch all OIDC providers (admin only)
 * EXTERNAL: GET /api/admin/oidc
 */
export async function listOidcProviders(): Promise<OidcProvider[]> {
  return apiClient.get<OidcProvider[]>('/admin/oidc');
}

/**
 * PURPOSE: Create a new OIDC provider (admin only)
 * EXTERNAL: POST /api/admin/oidc
 */
export async function createOidcProvider(data: CreateOidcProviderRequest): Promise<OidcProvider> {
  return apiClient.post<OidcProvider>('/admin/oidc', data);
}

/**
 * PURPOSE: Update an existing OIDC provider (admin only)
 * EXTERNAL: PUT /api/admin/oidc/:id
 */
export async function updateOidcProvider(id: string, data: UpdateOidcProviderRequest): Promise<OidcProvider> {
  return apiClient.put<OidcProvider>(`/admin/oidc/${id}`, data);
}

/**
 * PURPOSE: Delete an OIDC provider (admin only)
 * EXTERNAL: DELETE /api/admin/oidc/:id
 */
export async function deleteOidcProvider(id: string): Promise<void> {
  return apiClient.delete(`/admin/oidc/${id}`);
}

/**
 * PURPOSE: Fetch active OIDC providers for login page display (public)
 * CONSTRAINTS: Returns only public fields — no secrets or config details
 * EXTERNAL: GET /api/auth/oidc/providers
 */
export async function listLoginProviders(): Promise<OidcLoginProvider[]> {
  return apiClient.get<OidcLoginProvider[]>('/auth/oidc/providers');
}

/**
 * PURPOSE: Get the authorization URL to redirect the user to the OIDC provider
 * EXTERNAL: GET /api/auth/oidc/:id/authorize
 */
export async function getAuthorizeUrl(providerId: string): Promise<OidcAuthorizeResponse> {
  return apiClient.get<OidcAuthorizeResponse>(`/auth/oidc/${providerId}/authorize`);
}
