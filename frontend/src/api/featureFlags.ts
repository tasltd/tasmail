// TMAIL-165/166: feature-flag API client.
// Public endpoint feeds the SPA's signup/onboarding gating; admin endpoints power
// the runtime-toggle dashboard.
import { apiClient } from './client';

export interface FeatureFlag {
  key: string;
  name: string;
  description: string;
  enabled: boolean;
  value: unknown | null;
  is_public: boolean;
  updated_at: string | null;
  updated_by: string | null;
}

export const featureFlagsApi = {
  // Public endpoint — works without an auth token (used by /signup before login).
  listPublic: () => apiClient.get<FeatureFlag[]>('/feature-flags'),
  // Admin-only — every authenticated user can call until role gating ships.
  listAll: () => apiClient.get<FeatureFlag[]>('/admin/feature-flags'),
  update: (key: string, body: { enabled?: boolean; value?: unknown }) =>
    apiClient.patch<FeatureFlag>(`/admin/feature-flags/${key}`, body),
};
