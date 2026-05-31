// TMAIL-235 + TMAIL-236 + TMAIL-237: live calendar view backed by
// /api/calendar/events. The previous in-memory `initialEvents` array is gone;
// reads + writes hit the live PostgreSQL `calendar_events` table via the
// Axum backend. The "color" field is a UI-only convenience, derived
// deterministically from the event id so colors stay stable per row.
//
// TMAIL-351: extended with edit (reuses the create form via EventFormDialog),
// attendees chip input + free-busy column, RSVP responder when the user is
// an invitee, RRULE-based recurrence picker, ICS download, and the
// suggest-slots button. The legacy inline add-form was replaced with the
// shared dialog so create + edit go through one code path.
import { useMemo, useState } from 'react';
import { Link } from 'react-router';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Calendar } from '@/components/ui/calendar';
import { Button } from '@/components/ui/button';
import { Plus, Trash2, Clock, CalendarDays, ArrowLeft, Pencil } from 'lucide-react';
import {
  listEvents,
  cancelEvent,
  getEvent,
  type CalendarEvent,
  type CalendarEventWithAttendees,
} from '@/api/calendar';
import { EventFormDialog } from './EventFormDialog';

const EVENT_COLORS = [
  'bg-blue-500',
  'bg-green-500',
  'bg-red-500',
  'bg-purple-500',
  'bg-orange-500',
];

function colorFor(id: string): string {
  // Stable hash so the same event always gets the same dot color.
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) >>> 0;
  return EVENT_COLORS[h % EVENT_COLORS.length];
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function timeOf(iso: string): string {
  const d = new Date(iso);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function CalendarView() {
  const qc = useQueryClient();
  const [selectedDate, setSelectedDate] = useState<Date | undefined>(new Date());
  const [formMode, setFormMode] = useState<'closed' | 'create' | 'edit'>('closed');
  const [editingEventId, setEditingEventId] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [mobilePanel, setMobilePanel] = useState<'calendar' | 'day'>('calendar');

  // Window the query around selectedDate's month so we don't pull the entire
  // calendar history. Backend supports ?start= and ?end= ISO bounds.
  const monthBounds = useMemo(() => {
    const ref = selectedDate ?? new Date();
    const start = new Date(ref.getFullYear(), ref.getMonth() - 1, 1).toISOString();
    const end = new Date(ref.getFullYear(), ref.getMonth() + 2, 0, 23, 59, 59).toISOString();
    return { start, end };
  }, [selectedDate]);

  const eventsQ = useQuery<CalendarEvent[]>({
    queryKey: ['calendar', monthBounds.start, monthBounds.end],
    queryFn: () => listEvents(monthBounds.start, monthBounds.end),
  });

  // Fetch the full event-with-attendees only when we open the edit dialog.
  // Listing endpoint returns the bare event (no attendees), but edit needs
  // the attendee chips so we round-trip via GET /api/calendar/events/{id}.
  const editingEventQ = useQuery<CalendarEventWithAttendees>({
    queryKey: ['calendar', 'event', editingEventId],
    queryFn: () => getEvent(editingEventId as string),
    enabled: Boolean(editingEventId) && formMode === 'edit',
  });

  const deleteMut = useMutation({
    mutationFn: (id: string) => cancelEvent(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['calendar'] }),
    onError: (err: Error) => setActionError(err.message),
  });

  const events = eventsQ.data ?? [];

  const eventsOnSelectedDate = useMemo(
    () =>
      events.filter(
        (e) =>
          selectedDate &&
          new Date(e.start_time).toDateString() === selectedDate.toDateString(),
      ),
    [events, selectedDate],
  );

  const datesWithEvents = useMemo(
    () => events.map((e) => new Date(e.start_time)),
    [events],
  );

  function openCreate() {
    setEditingEventId(null);
    setFormMode('create');
    setMobilePanel('day');
  }

  function openEdit(id: string) {
    setEditingEventId(id);
    setFormMode('edit');
  }

  function handleDialogOpenChange(open: boolean) {
    if (!open) {
      setFormMode('closed');
      setEditingEventId(null);
    }
  }

  return (
    <div className="flex h-full bg-white dark:bg-zinc-950 overflow-hidden">
      {/* Left: Calendar Picker */}
      <div className={`
        w-full md:w-80 border-r border-zinc-200 dark:border-zinc-800 flex flex-col shrink-0
        ${mobilePanel === 'day' ? 'hidden md:flex' : 'flex'}
      `}>
        <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center px-4 gap-2">
          <Link to="/">
            <Button variant="ghost" size="icon" title="Back to Mail">
              <ArrowLeft className="size-4" />
            </Button>
          </Link>
          <CalendarDays className="size-5 text-blue-600" />
          <h2 className="font-semibold text-base flex-1">Calendar</h2>
        </div>

        <div className="p-3 sm:p-4">
          <Calendar
            mode="single"
            selected={selectedDate}
            onSelect={(date) => {
              setSelectedDate(date);
              setMobilePanel('day');
            }}
            modifiers={{ hasEvent: datesWithEvents }}
            modifiersClassNames={{
              hasEvent: 'underline decoration-blue-500 decoration-2 font-semibold',
            }}
            className="rounded-xl border border-zinc-200 dark:border-zinc-700 w-full"
          />
        </div>

        <div className="px-4 pb-4 flex gap-2">
          <Button
            className="flex-1"
            onClick={openCreate}
            disabled={!selectedDate}
            data-testid="new-event-button"
          >
            <Plus className="size-4 mr-2" />
            New Event
          </Button>
          <Button
            variant="outline"
            className="md:hidden"
            onClick={() => setMobilePanel('day')}
            disabled={!selectedDate}
          >
            View Day
          </Button>
        </div>

        <div className="flex-1 overflow-y-auto px-4 space-y-1">
          <p className="text-xs text-zinc-400 font-medium uppercase tracking-wide mb-2">Upcoming</p>
          {eventsQ.isLoading && <div className="text-xs text-zinc-500">Loading…</div>}
          {eventsQ.isError && (
            <div className="text-xs text-red-600">Couldn't load events</div>
          )}
          {events
            .slice()
            .sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime())
            .slice(0, 8)
            .map((e) => {
              const d = new Date(e.start_time);
              return (
                <div
                  key={e.id}
                  className="flex items-center gap-2 text-sm py-1 cursor-pointer hover:text-blue-600 transition-colors"
                  onClick={() => { setSelectedDate(d); setMobilePanel('day'); }}
                >
                  <span className={`size-2 rounded-full shrink-0 ${colorFor(e.id)}`} />
                  <span className="truncate flex-1">{e.title}</span>
                  <span className="text-xs text-zinc-400 shrink-0">
                    {d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}
                  </span>
                </div>
              );
            })}
        </div>
      </div>

      {/* Right: Day View */}
      <div className={`
        flex-1 flex flex-col overflow-hidden
        ${mobilePanel === 'day' ? 'flex' : 'hidden md:flex'}
      `}>
        <div className="h-14 border-b border-zinc-200 dark:border-zinc-800 flex items-center justify-between px-4 sm:px-6 gap-2">
          <button
            className="md:hidden text-blue-600 flex items-center gap-1 text-sm font-medium shrink-0"
            onClick={() => setMobilePanel('calendar')}
          >
            ← Cal
          </button>
          <div className="flex-1 min-w-0">
            <h3 className="font-semibold text-sm sm:text-base truncate">
              {selectedDate
                ? selectedDate.toLocaleDateString('en-US', {
                    weekday: 'long',
                    month: 'long',
                    day: 'numeric',
                    year: 'numeric',
                  })
                : 'Select a date'}
            </h3>
            <p className="text-xs text-zinc-400">
              {eventsOnSelectedDate.length} event{eventsOnSelectedDate.length !== 1 ? 's' : ''}
            </p>
          </div>
          <Button variant="outline" size="sm" onClick={() => { setSelectedDate(new Date()); setMobilePanel('day'); }} className="shrink-0">
            Today
          </Button>
        </div>

        {actionError && (
          <div className="mx-3 sm:mx-6 mt-3 p-3 rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 text-sm text-red-700 dark:text-red-300">
            {actionError}
          </div>
        )}

        <div className="flex-1 overflow-y-auto px-3 sm:px-6 py-3 sm:py-4 space-y-3">
          {eventsQ.isLoading ? (
            <div className="text-sm text-zinc-500 p-4">Loading events…</div>
          ) : eventsQ.isError ? (
            <div className="text-sm text-red-600 p-4">
              Couldn't load events. {String(eventsQ.error)}
            </div>
          ) : eventsOnSelectedDate.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-full text-center text-zinc-400 gap-3">
              <CalendarDays className="size-12 opacity-30" />
              <p className="text-sm">No events on this day</p>
              <Button variant="outline" size="sm" onClick={openCreate}>
                <Plus className="size-4 mr-1" /> Add Event
              </Button>
            </div>
          ) : (
            eventsOnSelectedDate
              .slice()
              .sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime())
              .map((event) => (
                <EventRow
                  key={event.id}
                  event={event}
                  onEdit={() => openEdit(event.id)}
                  onDelete={() => {
                    if (window.confirm(`Cancel "${event.title}"?`)) {
                      deleteMut.mutate(event.id);
                    }
                  }}
                  deleting={deleteMut.isPending}
                />
              ))
          )}
        </div>
      </div>

      {/* Shared create/edit dialog (TMAIL-351) */}
      <EventFormDialog
        open={formMode !== 'closed'}
        onOpenChange={handleDialogOpenChange}
        event={
          formMode === 'edit' && editingEventQ.data
            ? (editingEventQ.data as CalendarEventWithAttendees)
            : null
        }
        defaultDate={selectedDate ?? new Date()}
      />

    </div>
  );
}

// ---- Row component (kept in-file because it's a private presentational
//     unit and pulling it out would add boilerplate without a reuse story).

interface EventRowProps {
  event: CalendarEvent;
  onEdit: () => void;
  onDelete: () => void;
  deleting: boolean;
}

function EventRow({ event, onEdit, onDelete, deleting }: EventRowProps) {
  return (
    <div
      className="flex items-start gap-4 p-4 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 hover:shadow-md transition-shadow group"
      data-testid="event-row"
    >
      <div className={`w-1.5 self-stretch rounded-full shrink-0 ${colorFor(event.id)}`} />
      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-2">
          <h4 className="font-semibold text-sm truncate">{event.title}</h4>
          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100">
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-zinc-400 hover:text-blue-500"
              onClick={onEdit}
              data-testid="edit-event-button"
              title="Edit event"
            >
              <Pencil className="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7 text-zinc-400 hover:text-red-500"
              disabled={deleting}
              onClick={onDelete}
              title="Cancel event"
            >
              <Trash2 className="size-4" />
            </Button>
          </div>
        </div>
        <div className="flex items-center gap-1 text-xs text-zinc-400 mt-0.5">
          <Clock className="size-3" />
          <span>{timeOf(event.start_time)} – {timeOf(event.end_time)}</span>
          {event.recurrence_rule && (
            <span
              className="ml-2 text-[10px] uppercase tracking-wide text-blue-500"
              title={event.recurrence_rule}
            >
              repeats
            </span>
          )}
        </div>
        {event.location && (
          <p className="text-xs text-zinc-500 mt-1">📍 {event.location}</p>
        )}
        {event.description && (
          <p className="text-xs text-zinc-500 mt-1">{event.description}</p>
        )}
      </div>
    </div>
  );
}
