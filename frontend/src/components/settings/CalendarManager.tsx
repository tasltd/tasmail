// Added: Calendar/meeting manager component for TMAIL-127
// Changed: Added CalendarView toggle for visual calendar grid (TMAIL-118)
import React, { lazy, Suspense } from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, Calendar, Download, Check, X, HelpCircle, Users, MapPin, LayoutGrid, Link2, Copy } from 'lucide-react';
import {
  listEvents,
  createEvent,
  cancelEvent,
  rsvpEvent,
  getEvent,
  downloadEventIcs,
  updateEvent,
} from '../../api/calendar';
import type {
  CalendarEvent,
  CreateEventRequest,
} from '../../api/calendar';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';
// Added (TMAIL-269): builder for the public /book/:token share URL.
import { buildBookingUrl } from '../../api/public-calendar';
// Changed (TMAIL-259): CalendarView pulls in the full @fullcalendar/* family
// (~600 kB raw). Defer it until the user toggles into Grid mode so the
// list-mode entry into the Calendar manager is fast for the common case.
const CalendarView = lazy(() => import('./CalendarView').then((m) => ({ default: m.CalendarView })));

// Added: Status badge color mapping for event status display
const STATUS_COLORS: Record<string, string> = {
  tentative: '#f59e0b',
  confirmed: '#22c55e',
  cancelled: '#ef4444',
};

// Added: RSVP badge color mapping for attendee display
const RSVP_COLORS: Record<string, string> = {
  pending: '#6b7280',
  accepted: '#22c55e',
  declined: '#ef4444',
  maybe: '#f59e0b',
};

// Added: Attendee email input row for the create form
function AttendeeRow({
  email,
  onRemove,
}: {
  email: string;
  onRemove: () => void;
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '4px 0' }}>
      <span style={{ flex: 1, fontSize: '13px' }}>{email}</span>
      <button type="button" className="btn btn--icon" onClick={onRemove} title="Remove attendee">
        <X size={14} />
      </button>
    </div>
  );
}

// Added: Inline form for creating new calendar events
function EventForm({
  onSave,
  onCancel,
}: {
  onSave: (data: CreateEventRequest) => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [location, setLocation] = useState('');
  const [startTime, setStartTime] = useState('');
  const [endTime, setEndTime] = useState('');
  const [allDay, setAllDay] = useState(false);
  const [attendeeEmail, setAttendeeEmail] = useState('');
  const [attendees, setAttendees] = useState<{ email: string }[]>([]);

  const handleAddAttendee = () => {
    const trimmed = attendeeEmail.trim();
    if (trimmed && !attendees.some((a) => a.email === trimmed)) {
      setAttendees([...attendees, { email: trimmed }]);
      setAttendeeEmail('');
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      title,
      description: description || undefined,
      location: location || undefined,
      start_time: new Date(startTime).toISOString(),
      end_time: new Date(endTime).toISOString(),
      all_day: allDay,
      attendees: attendees.length > 0 ? attendees : undefined,
    });
  };

  return (
    <form className="composer__fields" onSubmit={handleSubmit} style={{ gap: '12px' }}>
      <div className="composer__field">
        <label>Title</label>
        <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Event title" required />
      </div>
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>Description</label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Optional description"
          rows={3}
          style={{ width: '100%', padding: '8px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontSize: '13px' }}
        />
      </div>
      <div className="composer__field">
        <label>Location</label>
        <input value={location} onChange={(e) => setLocation(e.target.value)} placeholder="Optional location" />
      </div>
      <div className="composer__field">
        <label>Start</label>
        <input type="datetime-local" value={startTime} onChange={(e) => setStartTime(e.target.value)} required />
      </div>
      <div className="composer__field">
        <label>End</label>
        <input type="datetime-local" value={endTime} onChange={(e) => setEndTime(e.target.value)} required />
      </div>
      <div className="composer__field">
        <label style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <input type="checkbox" checked={allDay} onChange={(e) => setAllDay(e.target.checked)} />
          All Day
        </label>
      </div>
      {/* Added: Attendee email input with add button */}
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>Attendees</label>
        <div style={{ display: 'flex', gap: '8px', width: '100%' }}>
          <input
            value={attendeeEmail}
            onChange={(e) => setAttendeeEmail(e.target.value)}
            placeholder="attendee@example.com"
            style={{ flex: 1 }}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                handleAddAttendee();
              }
            }}
          />
          <button type="button" className="btn" onClick={handleAddAttendee}>
            Add
          </button>
        </div>
        {attendees.map((a, index) => (
          <AttendeeRow
            key={a.email}
            email={a.email}
            onRemove={() => setAttendees(attendees.filter((_, i) => i !== index))}
          />
        ))}
      </div>
      <div className="composer__actions">
        <button type="submit" className="btn btn--primary">Create Event</button>
        <button type="button" className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </form>
  );
}

// Added: Event detail view showing attendees, RSVP buttons, and ICS download
function EventDetail({
  eventId,
  onBack,
}: {
  eventId: string;
  onBack: () => void;
}) {
  const queryClient = useQueryClient();

  const { data: eventDetail, isLoading } = useQuery({
    queryKey: ['calendar-event', eventId],
    queryFn: () => getEvent(eventId),
  });

  const rsvpMut = useMutation({
    mutationFn: ({ status }: { status: 'accepted' | 'declined' | 'maybe' }) =>
      rsvpEvent(eventId, { status }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['calendar-event', eventId] }),
  });

  // Added (TMAIL-269): toggle public scheduling and copy the share URL.
  const publicToggleMut = useMutation({
    mutationFn: (enabled: boolean) => updateEvent(eventId, { public_enabled: enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['calendar-event', eventId] }),
  });
  const [copyState, setCopyState] = useState<'idle' | 'copied'>('idle');

  const handleCopyShareLink = async () => {
    if (!eventDetail?.public_token) return;
    const url = buildBookingUrl(eventDetail.public_token);
    try {
      await navigator.clipboard.writeText(url);
    } catch {
      // Fallback for browsers/contexts without async clipboard (HTTP, old Firefox).
      const ta = document.createElement('textarea');
      ta.value = url;
      ta.setAttribute('readonly', 'true');
      ta.style.position = 'absolute';
      ta.style.left = '-9999px';
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
    setCopyState('copied');
    setTimeout(() => setCopyState('idle'), 1500);
  };

  const handleDownloadIcs = async () => {
    const icsContent = await downloadEventIcs(eventId);
    // Added: Create blob and trigger download for ICS file
    const blob = new Blob([icsContent], { type: 'text/calendar' });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = `event-${eventId}.ics`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  if (isLoading) return <LoadingSkeleton rows={6} />;
  if (!eventDetail) return <p>Event not found.</p>;

  return (
    <div>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={onBack} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>{eventDetail.title}</h2>
        <button className="btn" onClick={handleDownloadIcs} title="Download ICS">
          <Download size={16} /> ICS
        </button>
      </div>

      <div style={{ padding: '12px 0' }}>
        {eventDetail.description && (
          <p style={{ color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
            {eventDetail.description}
          </p>
        )}
        {eventDetail.location && (
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginBottom: '8px', fontSize: '13px' }}>
            <MapPin size={14} />
            {eventDetail.location}
          </div>
        )}
        <div style={{ fontSize: '13px', color: 'var(--color-text-secondary)', marginBottom: '8px' }}>
          {new Date(eventDetail.start_time).toLocaleString()} — {new Date(eventDetail.end_time).toLocaleString()}
        </div>
        <span style={{
          padding: '2px 8px',
          borderRadius: '4px',
          fontSize: '11px',
          fontWeight: 600,
          color: 'white',
          background: STATUS_COLORS[eventDetail.status] || STATUS_COLORS.tentative,
        }}>
          {eventDetail.status}
        </span>
      </div>

      {/* Added: RSVP action buttons */}
      <div style={{ display: 'flex', gap: '8px', margin: '12px 0', borderBottom: '1px solid var(--color-border)', paddingBottom: '12px' }}>
        <button className="btn btn--primary" onClick={() => rsvpMut.mutate({ status: 'accepted' })}>
          <Check size={14} /> Accept
        </button>
        <button className="btn" onClick={() => rsvpMut.mutate({ status: 'declined' })}>
          <X size={14} /> Decline
        </button>
        <button className="btn" onClick={() => rsvpMut.mutate({ status: 'maybe' })}>
          <HelpCircle size={14} /> Maybe
        </button>
      </div>

      {/* Added (TMAIL-269): public booking link controls.
          Owners flip public_enabled on to publish /book/{token} for external participants. */}
      <div
        data-testid="public-share-section"
        style={{
          margin: '12px 0',
          padding: '12px',
          border: '1px solid var(--color-border)',
          borderRadius: '6px',
          background: 'var(--color-background-subtle, transparent)',
        }}
      >
        <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', fontWeight: 600 }}>
          <input
            type="checkbox"
            checked={!!eventDetail.public_enabled}
            onChange={(e) => publicToggleMut.mutate(e.target.checked)}
            aria-label="Allow external participants to book via a share link"
          />
          <Link2 size={14} /> Public booking link
        </label>
        <p style={{ margin: '6px 0 8px', fontSize: '12px', color: 'var(--color-text-secondary)' }}>
          Anyone with this link can view the event and RSVP without signing in.
        </p>
        {eventDetail.public_enabled && eventDetail.public_token && (
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <code
              data-testid="public-share-url"
              style={{
                flex: 1,
                minWidth: '180px',
                padding: '6px 8px',
                fontSize: '12px',
                background: 'var(--color-surface, #f1f5f9)',
                border: '1px solid var(--color-border)',
                borderRadius: '4px',
                overflowX: 'auto',
                whiteSpace: 'nowrap',
              }}
            >
              {buildBookingUrl(eventDetail.public_token)}
            </code>
            <button
              type="button"
              className="btn"
              onClick={handleCopyShareLink}
              aria-label="Copy share link"
              data-testid="public-share-copy"
            >
              <Copy size={14} /> {copyState === 'copied' ? 'Copied!' : 'Copy'}
            </button>
          </div>
        )}
      </div>

      {/* Added: Attendee list with RSVP status badges */}
      <h3 style={{ fontSize: '14px', marginBottom: '8px' }}>
        <Users size={16} style={{ verticalAlign: 'middle', marginRight: '6px' }} />
        Attendees ({eventDetail.attendees.length})
      </h3>
      {eventDetail.attendees.length === 0 && (
        <p style={{ color: 'var(--color-text-secondary)', fontSize: '13px' }}>No attendees.</p>
      )}
      {eventDetail.attendees.map((attendee) => (
        <div
          key={attendee.id}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '6px 0',
            borderBottom: '1px solid var(--color-border)',
          }}
        >
          <span style={{ flex: 1, fontSize: '13px' }}>
            {attendee.display_name || attendee.email}
            {attendee.display_name && (
              <span style={{ color: 'var(--color-text-secondary)', marginLeft: '4px' }}>
                ({attendee.email})
              </span>
            )}
          </span>
          <span
            style={{
              padding: '1px 6px',
              borderRadius: '4px',
              fontSize: '11px',
              fontWeight: 600,
              color: 'white',
              background: RSVP_COLORS[attendee.rsvp] || RSVP_COLORS.pending,
            }}
          >
            {attendee.rsvp}
          </span>
        </div>
      ))}
    </div>
  );
}

export function CalendarManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null);
  // Added: Toggle between list view and visual calendar grid view (TMAIL-118)
  const [showCalendarView, setShowCalendarView] = useState(false);

  const { data: events, isLoading } = useQuery({
    queryKey: ['calendar-events'],
    queryFn: () => listEvents(),
  });

  const createMut = useMutation({
    mutationFn: createEvent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['calendar-events'] });
      setIsCreating(false);
    },
  });

  const cancelMut = useMutation({
    mutationFn: cancelEvent,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['calendar-events'] }),
  });

  // Added: Drag-and-drop reschedule handler — invalidates both the list and
  // the per-range view query so the grid stays in sync (TMAIL-118).
  const rescheduleMut = useMutation({
    mutationFn: ({ id, start, end }: { id: string; start: string; end: string }) =>
      updateEvent(id, { start_time: start, end_time: end }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['calendar-events'] });
      queryClient.invalidateQueries({ queryKey: ['calendar-events-view'] });
    },
  });

  if (isLoading) return <LoadingSkeleton rows={6} />;

  // Added: Show event detail when an event is selected
  if (selectedEventId) {
    return (
      <div style={{ padding: '16px', maxWidth: '800px' }}>
        <EventDetail eventId={selectedEventId} onBack={() => setSelectedEventId(null)} />
      </div>
    );
  }

  return (
    <div style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Calendar</h2>
        {/* Added: Calendar grid view toggle button (TMAIL-118) */}
        <button
          className={`btn ${showCalendarView ? 'btn--primary' : ''}`}
          onClick={() => setShowCalendarView(!showCalendarView)}
          title={showCalendarView ? 'Switch to list view' : 'Switch to calendar view'}
        >
          <LayoutGrid size={16} /> {showCalendarView ? 'List' : 'Grid'}
        </button>
        <button className="btn btn--primary" onClick={() => setIsCreating(true)}>
          <Plus size={16} /> New Event
        </button>
      </div>

      {/* Added: Visual calendar grid view (TMAIL-118)
          Changed (TMAIL-259): wrapped in Suspense — the calendar-vendor chunk
          (~600 kB raw, ~180 kB gzip for @fullcalendar/*) only loads when the
          user toggles into Grid mode, not on every Calendar Manager visit. */}
      {showCalendarView && (
        <div style={{ marginTop: '12px' }}>
          <Suspense fallback={<LoadingSkeleton />}>
            <CalendarView
              onSelectEvent={(eventId) => setSelectedEventId(eventId)}
              onCreateEvent={() => setIsCreating(true)}
              onRescheduleEvent={(id, start, end) => rescheduleMut.mutate({ id, start, end })}
            />
          </Suspense>
        </div>
      )}

      {isCreating && (
        <div style={{ marginTop: '12px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>New Event</h3>
          <EventForm onSave={(data) => createMut.mutate(data)} onCancel={() => setIsCreating(false)} />
        </div>
      )}

      <div style={{ marginTop: '12px' }}>
        {(!events || events.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            No events yet. Create one to get started.
          </p>
        )}
        {events?.map((event: CalendarEvent) => (
          <div
            key={event.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '10px 12px',
              borderBottom: '1px solid var(--color-border)',
              cursor: 'pointer',
              opacity: event.status === 'cancelled' ? 0.6 : 1,
            }}
            onClick={() => setSelectedEventId(event.id)}
          >
            <Calendar size={20} style={{ flexShrink: 0, color: 'var(--color-primary)' }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {event.title}
              </div>
              <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                {/* Added: Event status badge */}
                <span
                  style={{
                    padding: '1px 6px',
                    borderRadius: '4px',
                    fontSize: '11px',
                    fontWeight: 600,
                    color: 'white',
                    background: STATUS_COLORS[event.status] || STATUS_COLORS.tentative,
                  }}
                >
                  {event.status}
                </span>
                {/* Added: Event date/time display */}
                <span>
                  {new Date(event.start_time).toLocaleDateString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                </span>
                {/* Added: Location indicator */}
                {event.location && (
                  <span style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
                    <MapPin size={12} />
                    {event.location}
                  </span>
                )}
              </div>
            </div>
            <button
              className="btn btn--icon btn--danger"
              onClick={(e) => {
                e.stopPropagation();
                cancelMut.mutate(event.id);
              }}
              title="Cancel event"
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
