// Added (TMAIL-269): public scheduling API for the /book/{token} page.
//
// These endpoints are unauthenticated — the BookingPage is shown to external
// visitors who clicked a share link. We don't go through `apiClient` because
// that singleton attaches an Authorization header and 401-redirects to /login.
// Public routes need plain fetch with no auth header.

import { API_BASE_URL } from '../utils/constants';

// Slim event projection returned by GET /api/calendar/public/{token}.
// Mirrors `PublicEventSummary` in backend/src/handlers/public_calendar.rs.
export interface PublicEventSummary {
  id: string;
  title: string;
  description: string | null;
  location: string | null;
  start_time: string;
  end_time: string;
  all_day: boolean;
  status: string;
}

export type PublicRsvpStatus = 'accepted' | 'declined' | 'maybe';

export interface PublicRsvpRequest {
  email: string;
  name?: string;
  status: PublicRsvpStatus;
}

export interface PublicRsvpResponse {
  email: string;
  display_name: string | null;
  rsvp: string;
  responded_at: string | null;
}

// Thrown for any non-2xx response so the BookingPage can render an error
// instead of a blank screen. Message body is the server's plain-text response.
export class PublicCalendarError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message || `HTTP ${status}`);
    this.status = status;
    this.name = 'PublicCalendarError';
  }
}

/** GET /api/calendar/public/{token} — fetch the event summary for the booking page. */
export async function getPublicEvent(token: string): Promise<PublicEventSummary> {
  const res = await fetch(`${API_BASE_URL}/calendar/public/${encodeURIComponent(token)}`, {
    method: 'GET',
    headers: { Accept: 'application/json' },
  });
  if (!res.ok) {
    throw new PublicCalendarError(res.status, await res.text());
  }
  return (await res.json()) as PublicEventSummary;
}

/** POST /api/calendar/public/{token}/rsvp — record an external visitor's RSVP. */
export async function submitPublicRsvp(
  token: string,
  body: PublicRsvpRequest,
): Promise<PublicRsvpResponse> {
  const res = await fetch(`${API_BASE_URL}/calendar/public/${encodeURIComponent(token)}/rsvp`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    throw new PublicCalendarError(res.status, await res.text());
  }
  return (await res.json()) as PublicRsvpResponse;
}

/** Build the shareable URL an owner should send to external participants. */
export function buildBookingUrl(publicToken: string, origin?: string): string {
  const base = origin ?? (typeof window !== 'undefined' ? window.location.origin : '');
  return `${base}/book/${publicToken}`;
}
