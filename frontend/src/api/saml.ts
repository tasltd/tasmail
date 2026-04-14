// Added: SAML 2.0 SSO configuration API module for TMAIL-101
import { apiClient } from './client';

// Added: SAML configuration interface matching backend SamlConfiguration struct
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
  created_at: string;
  updated_at: string;
}

// Added: Create request — required fields for new SAML config
export interface CreateSamlConfigRequest {
  name: string;
  entity_id: string;
  sso_url: string;
  slo_url?: string;
  certificate: string;
  name_id_format?: string;
  attribute_mapping?: Record<string, string>;
}

// Added: Update request — all fields optional for partial updates
export interface UpdateSamlConfigRequest {
  name?: string;
  entity_id?: string;
  sso_url?: string;
  slo_url?: string;
  certificate?: string;
  name_id_format?: string;
  attribute_mapping?: Record<string, string>;
  active?: boolean;
}

// Added: SAML login redirect response
export interface SamlLoginResponse {
  redirect_url: string;
}

/**
 * PURPOSE: Fetch all SAML IdP configurations (admin only)
 * EXTERNAL: GET /api/admin/saml
 */
export async function listSamlConfigs(): Promise<SamlConfiguration[]> {
  return apiClient.get<SamlConfiguration[]>('/admin/saml');
}

/**
 * PURPOSE: Create a new SAML IdP configuration (admin only)
 * EXTERNAL: POST /api/admin/saml
 */
export async function createSamlConfig(data: CreateSamlConfigRequest): Promise<SamlConfiguration> {
  return apiClient.post<SamlConfiguration>('/admin/saml', data);
}

/**
 * PURPOSE: Update an existing SAML IdP configuration (admin only)
 * EXTERNAL: PUT /api/admin/saml/:id
 */
export async function updateSamlConfig(id: string, data: UpdateSamlConfigRequest): Promise<SamlConfiguration> {
  return apiClient.put<SamlConfiguration>(`/admin/saml/${id}`, data);
}

/**
 * PURPOSE: Delete a SAML IdP configuration (admin only)
 * EXTERNAL: DELETE /api/admin/saml/:id
 */
export async function deleteSamlConfig(id: string): Promise<void> {
  return apiClient.delete(`/admin/saml/${id}`);
}

/**
 * PURPOSE: Get the SAML login redirect URL for a specific IdP configuration
 * EXTERNAL: GET /api/auth/saml/:id/login
 */
export async function getSamlLoginUrl(id: string): Promise<SamlLoginResponse> {
  return apiClient.get<SamlLoginResponse>(`/auth/saml/${id}/login`);
}
