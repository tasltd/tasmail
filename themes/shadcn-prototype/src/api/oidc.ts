// Added (TMAIL-327): OIDC client for the Modern UI's native login screen.
// Mirrors frontend/src/api/oidc.ts but only exposes the public surface
// (listLoginProviders + getAuthorizeUrl) that the login form needs.
import { apiClient } from './client';

export interface OidcLoginProvider {
  id: string;
  name: string;
  icon_url: string | null;
  button_label: string | null;
}

export interface OidcAuthorizeResponse {
  authorize_url: string;
  state: string;
}

export async function listLoginProviders(): Promise<OidcLoginProvider[]> {
  return apiClient.get<OidcLoginProvider[]>('/auth/oidc/providers');
}

export async function getAuthorizeUrl(providerId: string): Promise<OidcAuthorizeResponse> {
  return apiClient.get<OidcAuthorizeResponse>(`/auth/oidc/${providerId}/authorize`);
}
