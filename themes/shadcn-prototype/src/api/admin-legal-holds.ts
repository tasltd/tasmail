// TMAIL-354: Modern UI Admin → Legal Holds sub-tab API client. CRUD against
// /api/admin/legal-holds — backed by the Rust handlers in
// backend/src/handlers/retention.rs (legal_holds share that file with
// retention policies on the backend side).
import { apiClient } from './client';

export interface LegalHold {
  id: string;
  user_id: string;
  reason: string;
  placed_by: string;
  active: boolean;
  created_at: string;
  released_at: string | null;
}

export interface CreateLegalHoldRequest {
  user_id: string;
  reason: string;
}

export const adminLegalHoldsApi = {
  list: () => apiClient.get<LegalHold[]>('/admin/legal-holds'),
  create: (body: CreateLegalHoldRequest) =>
    apiClient.post<LegalHold>('/admin/legal-holds', body),
  // Backend uses PUT /api/admin/legal-holds/{id}/release with no body to
  // release a hold; we expose it as a dedicated method so callers don't
  // have to remember the verb.
  release: (id: string) =>
    apiClient.put<LegalHold>(`/admin/legal-holds/${id}/release`, {}),
};
