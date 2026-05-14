// TMAIL-235: ported calendar client so the alt-UI CalendarView can hit the
// same /api/calendar/events endpoints as the classic SPA.
import { apiClient } from './client';

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

export async function cancelEvent(id: string): Promise<void> {
  await apiClient.delete(`/calendar/events/${id}`);
}
