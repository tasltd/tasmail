// TMAIL-235: ported calendar client so the alt-UI CalendarView can hit the
// same /api/calendar/events endpoints as the classic SPA.
// TMAIL-351: added updateEvent, rsvpEvent, downloadEventIcs, getFreeBusy,
// suggestSlots so the Modern UI can edit, RSVP, export ICS, and suggest
// meeting slots without falling back to the classic SPA.
import { apiClient } from './client';
import { API_BASE_URL } from './constants';

export interface CalendarEvent {
  id: string;
  organizer_id: string;
  title: string;
  description: string | null;
  location: string | null;
  start_time: string;
  end_time: string;
  all_day: boolean;
  recurrence_rule: string | null;
  status: string;
  linked_message_uid: number | null;
  linked_folder: string | null;
  ics_uid: string;
  created_at: string;
  updated_at: string;
}

export interface EventAttendee {
  id: string;
  event_id: string;
  email: string;
  display_name: string | null;
  rsvp: string;
  responded_at: string | null;
}

export interface CalendarEventWithAttendees extends CalendarEvent {
  attendees: EventAttendee[];
}

export interface CreateEventRequest {
  title: string;
  description?: string;
  location?: string;
  start_time: string;
  end_time: string;
  all_day?: boolean;
  recurrence_rule?: string;
  attendees?: { email: string; display_name?: string }[];
  linked_message_uid?: number;
  linked_folder?: string;
}

// Added (TMAIL-351): mirror of backend UpdateEventRequest. Every field is
// optional — only what's set is sent; the rest stays untouched on the row.
export interface UpdateEventRequest {
  title?: string;
  description?: string | null;
  location?: string | null;
  start_time?: string;
  end_time?: string;
  all_day?: boolean;
  recurrence_rule?: string | null;
  status?: 'tentative' | 'confirmed' | 'cancelled';
}

// Added (TMAIL-351): RSVP body. Backend validates against this exact union.
export interface RsvpRequest {
  status: 'accepted' | 'declined' | 'maybe';
}

export async function listEvents(start?: string, end?: string): Promise<CalendarEvent[]> {
  const params = new URLSearchParams();
  if (start) params.set('start', start);
  if (end) params.set('end', end);
  const query = params.toString();
  return apiClient.get<CalendarEvent[]>(`/calendar/events${query ? `?${query}` : ''}`);
}

export async function createEvent(data: CreateEventRequest): Promise<CalendarEventWithAttendees> {
  return apiClient.post<CalendarEventWithAttendees>('/calendar/events', data);
}

export async function getEvent(id: string): Promise<CalendarEventWithAttendees> {
  return apiClient.get<CalendarEventWithAttendees>(`/calendar/events/${id}`);
}

// Added (TMAIL-351): PUT /api/calendar/events/{id} — partial update. Router
// is registered as PUT in backend/src/router.rs:554 (the issue text says
// PATCH; the in-code verb is PUT, kept stable for the classic SPA).
export async function updateEvent(
  id: string,
  data: UpdateEventRequest,
): Promise<CalendarEvent> {
  return apiClient.put<CalendarEvent>(`/calendar/events/${id}`, data);
}

export async function cancelEvent(id: string): Promise<void> {
  await apiClient.delete(`/calendar/events/${id}`);
}

// Added (TMAIL-351): RSVP — backend uses the authenticated user's email
// from the JWT, so the caller only sends the response choice.
export async function rsvpEvent(id: string, data: RsvpRequest): Promise<EventAttendee> {
  return apiClient.post<EventAttendee>(`/calendar/events/${id}/rsvp`, data);
}

// Added (TMAIL-351): ICS download. Backend returns text/calendar with
// Content-Disposition: attachment, so we bypass the JSON-aware apiClient
// and stream the body straight into a Blob → object URL for download.
// Returns the URL + filename so callers can wire an <a download> click.
export async function downloadEventIcs(id: string): Promise<{ url: string; filename: string }> {
  const token = apiClient.getToken();
  const resp = await fetch(`${API_BASE_URL}/calendar/events/${id}/ics`, {
    headers: token ? { Authorization: `Bearer ${token}` } : undefined,
  });
  if (!resp.ok) {
    throw new Error(`Failed to download ICS (${resp.status})`);
  }
  // Parse the filename from Content-Disposition; fall back to a sensible
  // default if the header is missing or malformed.
  const disposition = resp.headers.get('content-disposition') ?? '';
  const match = /filename="?([^";]+)"?/i.exec(disposition);
  const filename = match?.[1] ?? `event-${id}.ics`;
  const blob = await resp.blob();
  const url = URL.createObjectURL(blob);
  return { url, filename };
}

// ---- Free-busy + suggest-slots (TMAIL-351) -------------------------------

export interface FreeBusyRequest {
  attendees: string[];
  range_start: string;
  range_end: string;
}

export interface BusySpan {
  start: string;
  end: string;
}

export interface AttendeeBusy {
  email: string;
  status: 'resolved' | 'not_resolved';
  busy: BusySpan[];
}

export interface FreeBusyResponse {
  attendees: AttendeeBusy[];
}

export interface SuggestSlotsRequest {
  attendees: string[];
  duration_minutes: number;
  range_start: string;
  range_end: string;
  working_start_minute?: number;
  working_end_minute?: number;
  include_weekends?: boolean;
  max_slots?: number;
  step_minutes?: number;
}

export interface SuggestedSlot {
  start: string;
  end: string;
}

export interface SuggestSlotsResponse {
  slots: SuggestedSlot[];
  unresolved_attendees: string[];
}

export async function getFreeBusy(req: FreeBusyRequest): Promise<FreeBusyResponse> {
  return apiClient.post<FreeBusyResponse>('/calendar/free-busy', req);
}

export async function suggestSlots(req: SuggestSlotsRequest): Promise<SuggestSlotsResponse> {
  return apiClient.post<SuggestSlotsResponse>('/calendar/suggest-slots', req);
}
