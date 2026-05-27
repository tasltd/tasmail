// Added: Calendar API module for meeting scheduling (TMAIL-127)
import { apiClient } from './client';

// Added: Calendar event interface matching backend CalendarEvent struct
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
  // Added (TMAIL-269): public scheduling token + opt-in flag.
  public_token: string;
  public_enabled: boolean;
  created_at: string;
  updated_at: string;
}

// Added: Attendee interface matching backend EventAttendee struct
export interface EventAttendee {
  id: string;
  event_id: string;
  email: string;
  display_name: string | null;
  rsvp: string;
  responded_at: string | null;
}

// Added: Combined event with attendees for detail responses
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

export interface UpdateEventRequest {
  title?: string;
  description?: string;
  location?: string;
  start_time?: string;
  end_time?: string;
  all_day?: boolean;
  recurrence_rule?: string;
  status?: string;
  // Added (TMAIL-269): toggle the public booking page on/off.
  public_enabled?: boolean;
}

export interface RsvpRequest {
  status: 'accepted' | 'declined' | 'maybe';
}

/// PURPOSE: List calendar events with optional date range filter
export async function listEvents(start?: string, end?: string): Promise<CalendarEvent[]> {
  const params = new URLSearchParams();
  if (start) params.set('start', start);
  if (end) params.set('end', end);
  const query = params.toString();
  return apiClient.get<CalendarEvent[]>(`/calendar/events${query ? `?${query}` : ''}`);
}

/// PURPOSE: Create a new calendar event with attendees
export async function createEvent(data: CreateEventRequest): Promise<CalendarEventWithAttendees> {
  return apiClient.post<CalendarEventWithAttendees>('/calendar/events', data);
}

/// PURPOSE: Get a single event with its attendees
export async function getEvent(id: string): Promise<CalendarEventWithAttendees> {
  return apiClient.get<CalendarEventWithAttendees>(`/calendar/events/${id}`);
}

/// PURPOSE: Update an existing event
export async function updateEvent(id: string, data: UpdateEventRequest): Promise<CalendarEvent> {
  return apiClient.put<CalendarEvent>(`/calendar/events/${id}`, data);
}

/// PURPOSE: Cancel (delete) an event
export async function cancelEvent(id: string): Promise<void> {
  await apiClient.delete(`/calendar/events/${id}`);
}

/// PURPOSE: RSVP to an event (accept, decline, or maybe)
export async function rsvpEvent(id: string, data: RsvpRequest): Promise<EventAttendee> {
  return apiClient.post<EventAttendee>(`/calendar/events/${id}/rsvp`, data);
}

/// PURPOSE: Download ICS file for an event
export async function downloadEventIcs(id: string): Promise<string> {
  return apiClient.get<string>(`/calendar/events/${id}/ics`);
}
