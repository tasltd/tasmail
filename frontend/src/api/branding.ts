// Added: Branding API module for white-label customization (TMAIL-111)
import { apiClient } from './client';

// Added: Branding response interface matching backend Branding struct
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

// Added: Update request — all fields optional for partial updates
export interface UpdateBrandingRequest {
  app_name?: string;
  logo_url?: string;
  favicon_url?: string;
  primary_color?: string;
  secondary_color?: string;
  accent_color?: string;
  login_background_url?: string;
  custom_css?: string;
  footer_text?: string;
  support_email?: string;
  support_url?: string;
}

/**
 * PURPOSE: Fetch current branding configuration (public endpoint, no auth required)
 * EXTERNAL: GET /api/branding
 */
export async function getBranding(): Promise<Branding> {
  return apiClient.get<Branding>('/branding');
}

/**
 * PURPOSE: Update branding settings (admin only)
 * EXTERNAL: PUT /api/admin/branding
 */
export async function updateBranding(data: UpdateBrandingRequest): Promise<Branding> {
  return apiClient.put<Branding>('/admin/branding', data);
}

/**
 * PURPOSE: Reset branding to factory defaults (admin only)
 * EXTERNAL: POST /api/admin/branding/reset
 */
export async function resetBranding(): Promise<Branding> {
  return apiClient.post<Branding>('/admin/branding/reset');
}
