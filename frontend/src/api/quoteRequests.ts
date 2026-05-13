// TMAIL-185: admin client for the enterprise quote-request inbox.
import { apiClient } from './client';

export type QuoteStatus = 'new' | 'contacted' | 'quoted' | 'won' | 'lost';

export interface QuoteRequest {
  id: string;
  contact_name: string;
  contact_email: string;
  company: string | null;
  estimated_users: number | null;
  message: string;
  status: QuoteStatus;
  internal_notes: string | null;
  assigned_to: string | null;
  created_at: string | null;
  updated_at: string | null;
  contacted_at: string | null;
  quoted_at: string | null;
  closed_at: string | null;
}

export interface QuoteListResponse {
  items: QuoteRequest[];
  total: number;
  limit: number;
  offset: number;
}

export interface StatusCount {
  status: QuoteStatus;
  count: number;
}

// Added: TMAIL-206 — public submission shape (subset of QuoteRequest the form sends).
export interface QuoteSubmitBody {
  contact_name: string;
  contact_email: string;
  company?: string;
  estimated_users?: number;
  message: string;
}

export interface QuoteSubmitResponse {
  id: string;
  status: QuoteStatus;
}

export const quoteRequestsApi = {
  // Added: TMAIL-206 — public submission via apiClient so the landing-page form
  // picks up the same base-URL + retry plumbing every other client uses. The
  // endpoint itself is public; apiClient simply omits the Authorization header
  // when no access token is set.
  submit: (body: QuoteSubmitBody) =>
    apiClient.post<QuoteSubmitResponse>('/enterprise/quote-request', body),
  list: (status?: QuoteStatus, limit = 50, offset = 0) => {
    const params = new URLSearchParams();
    if (status) params.set('status', status);
    params.set('limit', String(limit));
    params.set('offset', String(offset));
    return apiClient.get<QuoteListResponse>(`/admin/quote-requests?${params.toString()}`);
  },
  stats: () => apiClient.get<StatusCount[]>('/admin/quote-requests/stats'),
  get: (id: string) => apiClient.get<QuoteRequest>(`/admin/quote-requests/${id}`),
  update: (id: string, body: { status?: QuoteStatus; internal_notes?: string; assigned_to?: string }) =>
    apiClient.patch<QuoteRequest>(`/admin/quote-requests/${id}`, body),
};
