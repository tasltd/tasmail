// Changed: Replaced custom pure-React grid with FullCalendar (TMAIL-118)
// Provides month / week / day / agenda (list) views, drag-and-drop rescheduling,
// and drag-to-create selection. RFC 5545 RRULE recurrence rendering is deferred —
// the backend already stores recurrence_rule, but the @fullcalendar/rrule plugin
// has an ESM interop bug with rrule@^2.8 (RRule + rrulestr exports come back
// undefined). Tracked as a follow-up; for now the master event is shown once.
import { useMemo, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import FullCalendar from '@fullcalendar/react';
import type { EventInput, EventClickArg, EventDropArg, DateSelectArg, DatesSetArg } from '@fullcalendar/core';
import dayGridPlugin from '@fullcalendar/daygrid';
import timeGridPlugin from '@fullcalendar/timegrid';
import listPlugin from '@fullcalendar/list';
import interactionPlugin from '@fullcalendar/interaction';
import type { EventResizeDoneArg } from '@fullcalendar/interaction';
import { listEvents } from '../../api/calendar';
import type { CalendarEvent } from '../../api/calendar';

// Added: Event chip background colour by status
const EVENT_COLORS: Record<string, string> = {
  confirmed: '#22c55e',
  tentative: '#f59e0b',
  cancelled: '#ef4444',
};

// PURPOSE: Map a TASMail CalendarEvent → FullCalendar EventInput. Recurring events
// are passed through with their RRULE string so the @fullcalendar/rrule plugin
// expands occurrences for the visible date range.
function toFullCalendarEvent(evt: CalendarEvent): EventInput {
  const color = EVENT_COLORS[evt.status] || EVENT_COLORS.tentative;
  const base: EventInput = {
    id: evt.id,
    title: evt.title,
    backgroundColor: color,
    borderColor: color,
    allDay: evt.all_day,
    extendedProps: {
      status: evt.status,
      description: evt.description,
      location: evt.location,
    },
  };
  // Added: Always render as a single non-recurring event for now (see header note)
  base.start = evt.start_time;
  base.end = evt.end_time;
  return base;
}

interface CalendarViewProps {
  onSelectEvent: (eventId: string) => void;
  onCreateEvent: (date?: Date) => void;
  // Added: Called when an event is dragged to a new time (drop) or resized.
  // Receives ISO start/end so the parent can call updateEvent().
  onRescheduleEvent?: (eventId: string, start: string, end: string) => void;
}

// PURPOSE: FullCalendar-backed calendar grid. The visible date range is tracked
// so we only fetch events for what's on screen — datesSet fires whenever the
// user navigates or switches view.
export function CalendarView({ onSelectEvent, onCreateEvent, onRescheduleEvent }: CalendarViewProps) {
  const calendarRef = useRef<FullCalendar | null>(null);

  // Added: Track the visible window. Default = current month so the first fetch
  // matches what FullCalendar will render on mount.
  const initialRange = useMemo(() => {
    const now = new Date();
    const start = new Date(now.getFullYear(), now.getMonth(), 1);
    const end = new Date(now.getFullYear(), now.getMonth() + 1, 0, 23, 59, 59);
    return { start: start.toISOString(), end: end.toISOString() };
  }, []);

  // NOTE: We store the range in a ref-like state through React Query's queryKey —
  // querying directly with a useState would force two renders. Instead we use a
  // ref + manual refetch on datesSet for simplicity.
  const rangeRef = useRef(initialRange);

  const { data: events, refetch } = useQuery({
    queryKey: ['calendar-events-view', rangeRef.current.start, rangeRef.current.end],
    queryFn: () => listEvents(rangeRef.current.start, rangeRef.current.end),
  });

  const fcEvents: EventInput[] = useMemo(
    () => (events ?? []).map(toFullCalendarEvent),
    [events],
  );

  // Added: Re-fetch whenever FullCalendar swaps the visible range (view change /
  // prev / next / today). Keeps the data layer aligned with the view.
  const handleDatesSet = (arg: DatesSetArg) => {
    const start = arg.start.toISOString();
    const end = arg.end.toISOString();
    if (rangeRef.current.start === start && rangeRef.current.end === end) return;
    rangeRef.current = { start, end };
    refetch();
  };

  const handleEventClick = (arg: EventClickArg) => {
    if (arg.event.id) onSelectEvent(arg.event.id);
  };

  const handleSelect = (arg: DateSelectArg) => {
    onCreateEvent(arg.start);
    // Added: Clear the selection so the highlighted range doesn't linger
    calendarRef.current?.getApi().unselect();
  };

  const handleEventDrop = (arg: EventDropArg) => {
    if (!onRescheduleEvent || !arg.event.id || !arg.event.start) {
      arg.revert();
      return;
    }
    const end = arg.event.end ?? arg.event.start;
    onRescheduleEvent(arg.event.id, arg.event.start.toISOString(), end.toISOString());
  };

  const handleEventResize = (arg: EventResizeDoneArg) => {
    if (!onRescheduleEvent || !arg.event.id || !arg.event.start || !arg.event.end) {
      arg.revert();
      return;
    }
    onRescheduleEvent(arg.event.id, arg.event.start.toISOString(), arg.event.end.toISOString());
  };

  return (
    <div data-testid="calendar-view">
      <FullCalendar
        ref={calendarRef}
        plugins={[dayGridPlugin, timeGridPlugin, listPlugin, interactionPlugin]}
        initialView="dayGridMonth"
        headerToolbar={{
          left: 'prev,next today',
          center: 'title',
          right: 'dayGridMonth,timeGridWeek,timeGridDay,listWeek',
        }}
        buttonText={{
          today: 'Today',
          month: 'Month',
          week: 'Week',
          day: 'Day',
          list: 'Agenda',
        }}
        height="auto"
        events={fcEvents}
        editable={Boolean(onRescheduleEvent)}
        selectable
        selectMirror
        dayMaxEvents={3}
        nowIndicator
        eventClick={handleEventClick}
        select={handleSelect}
        eventDrop={handleEventDrop}
        eventResize={handleEventResize}
        datesSet={handleDatesSet}
      />
    </div>
  );
}
