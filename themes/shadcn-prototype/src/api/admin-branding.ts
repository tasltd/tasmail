// TMAIL-353: Modern UI branding API client. Mirrors frontend/src/api/branding.ts
// so the alt-UI Admin → Branding sub-tab can read + write the public branding
// row (`/api/branding` GET) and the admin-only mutation endpoints
// (`/api/admin/branding` PUT, `/api/admin/branding/reset` POST).
import { apiClient } from './client';

export interface Branding {
  id: string;
  app_name: string;
  logo_url: string | null;
  favicon_url: string | null;
  primary_color: string;
  secondary_color: string;
  accent_color: string;
  login_background_url: string | null;
  custom_css: string | null;
  footer_text: string | null;
  support_email: string | null;
  support_url: string | null;
  updated_at: string;
}

export interface UpdateBrandingRequest {
  app_name?: string;
  logo_url?: string | null;
  favicon_url?: string | null;
  primary_color?: string;
  secondary_color?: string;
  accent_color?: string;
  login_background_url?: string | null;
  custom_css?: string | null;
  footer_text?: string | null;
  support_email?: string | null;
  support_url?: string | null;
}

export const adminBrandingApi = {
  get: () => apiClient.get<Branding>('/branding'),
  update: (body: UpdateBrandingRequest) =>
    apiClient.put<Branding>('/admin/branding', body),
  reset: () => apiClient.post<Branding>('/admin/branding/reset'),
};
