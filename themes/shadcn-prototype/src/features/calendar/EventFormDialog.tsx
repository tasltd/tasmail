// TMAIL-351: shared create/edit dialog backed by a single form. The dialog
// hosts the attendees chip input, recurrence picker, ICS download (edit
// mode only) and the "Suggest slots" panel. Reusing one form for both
// modes keeps create/edit behaviour identical — the only switch is which
// mutation runs on submit.
import { useEffect, useMemo, useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Download } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  createEvent,
  updateEvent,
  downloadEventIcs,
  type CalendarEvent,
  type CalendarEventWithAttendees,
  type CreateEventRequest,
  type EventAttendee,
  type UpdateEventRequest,
} from '@/api/calendar';
import { AttendeesField } from './AttendeesField';
import { SuggestSlotsPanel } from './SuggestSlotsPanel';
import { RsvpButtons } from './RsvpButtons';
import { currentUserEmail } from './currentUser';
import {
  RECURRENCE_PRESETS,
  presetForRrule,
  resolveRrule,
} from './recurrence';

export interface EventFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** When set, the form runs in EDIT mode and PUTs the changes to that
   *  event. When undefined, it runs in CREATE mode against the supplied
   *  defaultDate. */
  event?: CalendarEventWithAttendees | null;
  /** Date the create-form should anchor on when no event is supplied. */
  defaultDate: Date;
}

function pad(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function timeFromIso(iso: string): string {
  const d = new Date(iso);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function durationFromIsoPair(startIso: string, endIso: string): number {
  return Math.max(
    5,
    Math.round((new Date(endIso).getTime() - new Date(startIso).getTime()) / 60_000),
  );
}

function buildStartEnd(date: Date, time: string, durationMin: number): { start: Date; end: Date } {
  const [hh, mm] = time.split(':').map((s) => parseInt(s, 10));
  const start = new Date(date);
  start.setHours(hh || 0, mm || 0, 0, 0);
  const end = new Date(start.getTime() + durationMin * 60_000);
  return { start, end };
}

export function EventFormDialog({
  open,
  onOpenChange,
  event,
  defaultDate,
}: EventFormDialogProps) {
  const qc = useQueryClient();
  const isEdit = Boolean(event);

  const [title, setTitle] = useState('');
  const [time, setTime] = useState('09:00');
  const [durationMin, setDurationMin] = useState(60);
  const [description, setDescription] = useState('');
  const [location, setLocation] = useState('');
  const [attendees, setAttendees] = useState<string[]>([]);
  const [recurrencePreset, setRecurrencePreset] = useState<string>('none');
  const [customRrule, setCustomRrule] = useState('');
  const [error, setError] = useState<string | null>(null);

  // Re-seed the form whenever the dialog opens with a different event.
  useEffect(() => {
    if (!open) return;
    setError(null);
    if (event) {
      setTitle(event.title);
      setTime(timeFromIso(event.start_time));
      setDurationMin(durationFromIsoPair(event.start_time, event.end_time));
      setDescription(event.description ?? '');
      setLocation(event.location ?? '');
      setAttendees((event.attendees ?? []).map((a: EventAttendee) => a.email));
      const preset = presetForRrule(event.recurrence_rule);
      setRecurrencePreset(preset);
      setCustomRrule(preset === 'custom' ? event.recurrence_rule ?? '' : '');
    } else {
      setTitle('');
      setTime('09:00');
      setDurationMin(60);
      setDescription('');
      setLocation('');
      setAttendees([]);
      setRecurrencePreset('none');
      setCustomRrule('');
    }
  }, [open, event]);

  // Window used for free-busy / suggest-slots lookups. Anchored on the
  // anchor date so the free-busy column reflects the slot being edited.
  const { isoRangeStart, isoRangeEnd, isoAnchorStart, isoAnchorEnd } = useMemo(() => {
    const anchor = event ? new Date(event.start_time) : defaultDate;
    const { start: anchorStart, end: anchorEnd } = buildStartEnd(anchor, time, durationMin);
    // Two-week window for suggest-slots (matches backend MAX_SUGGEST_RANGE_DAYS).
    const range = new Date(anchor);
    range.setHours(0, 0, 0, 0);
    const rangeEndDate = new Date(range);
    rangeEndDate.setDate(rangeEndDate.getDate() + 14);
    return {
      isoRangeStart: range.toISOString(),
      isoRangeEnd: rangeEndDate.toISOString(),
      isoAnchorStart: anchorStart.toISOString(),
      isoAnchorEnd: anchorEnd.toISOString(),
    };
  }, [event, defaultDate, time, durationMin]);

  const createMut = useMutation({
    mutationFn: (body: CreateEventRequest) => createEvent(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['calendar'] });
      onOpenChange(false);
    },
    onError: (err: Error) => setError(err.message),
  });

  const updateMut = useMutation({
    mutationFn: ({ id, body }: { id: string; body: UpdateEventRequest }) =>
      updateEvent(id, body),
    onSuccess: (_data: CalendarEvent) => {
      qc.invalidateQueries({ queryKey: ['calendar'] });
      onOpenChange(false);
    },
    onError: (err: Error) => setError(err.message),
  });

  const icsMut = useMutation({
    mutationFn: (id: string) => downloadEventIcs(id),
    onSuccess: ({ url, filename }) => {
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      a.remove();
      // NOTE: Revoke after the click so the browser has time to start the
      // download. 30s is more than enough for any reasonable browser.
      setTimeout(() => URL.revokeObjectURL(url), 30_000);
    },
    onError: (err: Error) => setError(err.message),
  });

  function handleSave() {
    if (!title.trim()) {
      setError('Title is required');
      return;
    }
    const anchor = event ? new Date(event.start_time) : defaultDate;
    const { start, end } = buildStartEnd(anchor, time, durationMin);
    const rrule = resolveRrule(recurrencePreset, customRrule);
    if (isEdit && event) {
      const body: UpdateEventRequest = {
        title: title.trim(),
        description: description.trim() || null,
        location: location.trim() || null,
        start_time: start.toISOString(),
        end_time: end.toISOString(),
        recurrence_rule: rrule,
      };
      updateMut.mutate({ id: event.id, body });
    } else {
      const body: CreateEventRequest = {
        title: title.trim(),
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        start_time: start.toISOString(),
        end_time: end.toISOString(),
        all_day: false,
        recurrence_rule: rrule ?? undefined,
        attendees: attendees.filter((e) => e.trim() !== '').map((email) => ({ email })),
      };
      createMut.mutate(body);
    }
  }

  function handleSuggestPick(slot: { start: string; end: string }) {
    setTime(timeFromIso(slot.start));
    setDurationMin(durationFromIsoPair(slot.start, slot.end));
  }

  const saving = createMut.isPending || updateMut.isPending;

  // Render the RSVP buttons when (a) we're in edit mode (so we have the
  // attendees list) AND (b) the current user is one of the invitees. The
  // organizer doesn't get an RSVP because the backend identifies attendee
  // rows by email; organizers are usually absent from the attendee list.
  const me = currentUserEmail();
  const myAttendee = useMemo(() => {
    if (!isEdit || !event || !me) return null;
    return event.attendees.find((a) => a.email.toLowerCase() === me.toLowerCase()) ?? null;
  }, [isEdit, event, me]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-xl max-h-[90vh] overflow-y-auto" data-testid="event-form-dialog">
        <DialogHeader>
          <DialogTitle>{isEdit ? 'Edit event' : 'New event'}</DialogTitle>
        </DialogHeader>

        {error && (
          <div className="rounded-lg border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950 text-sm text-red-700 dark:text-red-300 p-2">
            {error}
          </div>
        )}

        <div className="space-y-3">
          <div>
            <label className="text-xs text-zinc-500 block mb-1">Title</label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              autoFocus
              data-testid="event-title-input"
              className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
            />
          </div>

          <div className="flex gap-3">
            <div className="flex-1">
              <label className="text-xs text-zinc-500 block mb-1">Time</label>
              <input
                type="time"
                value={time}
                onChange={(e) => setTime(e.target.value)}
                className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
            <div className="flex-1">
              <label className="text-xs text-zinc-500 block mb-1">Duration (min)</label>
              <input
                type="number"
                min={5}
                step={5}
                value={durationMin}
                onChange={(e) => setDurationMin(parseInt(e.target.value || '60', 10))}
                className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          <div>
            <label className="text-xs text-zinc-500 block mb-1">Repeat</label>
            <select
              value={recurrencePreset}
              data-testid="recurrence-select"
              onChange={(e) => setRecurrencePreset(e.target.value)}
              className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
            >
              {RECURRENCE_PRESETS.map((p) => (
                <option key={p.value} value={p.value}>{p.label}</option>
              ))}
            </select>
            {recurrencePreset === 'custom' && (
              <input
                type="text"
                value={customRrule}
                onChange={(e) => setCustomRrule(e.target.value)}
                placeholder="FREQ=MONTHLY;BYDAY=2MO"
                className="mt-1 w-full px-3 py-2 text-xs font-mono rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
              />
            )}
          </div>

          <input
            type="text"
            placeholder="Location (optional)"
            value={location}
            onChange={(e) => setLocation(e.target.value)}
            className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500"
          />

          <textarea
            placeholder="Description (optional)"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={2}
            className="w-full px-3 py-2 text-sm rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 outline-none focus:ring-2 focus:ring-blue-500 resize-none"
          />

          {myAttendee && event && (
            <div className="rounded-lg border border-blue-200 dark:border-blue-900 bg-blue-50 dark:bg-blue-950 p-2.5">
              <p className="text-xs text-blue-700 dark:text-blue-300 mb-1">
                You're invited to this event. Your current response:{' '}
                <strong>{myAttendee.rsvp}</strong>
              </p>
              <RsvpButtons eventId={event.id} currentRsvp={myAttendee.rsvp} />
            </div>
          )}

          <AttendeesField
            attendees={attendees}
            onChange={setAttendees}
            rangeStart={isoAnchorStart}
            rangeEnd={isoAnchorEnd}
            disabled={isEdit}
          />
          {isEdit && (
            <p className="text-[11px] text-zinc-400 -mt-1.5">
              Editing the attendee list of an existing event is coming soon. For
              now, cancel and recreate to change the roster.
            </p>
          )}

          <SuggestSlotsPanel
            attendees={attendees}
            durationMinutes={durationMin}
            rangeStart={isoRangeStart}
            rangeEnd={isoRangeEnd}
            onPick={handleSuggestPick}
            disabled={attendees.length === 0}
          />
        </div>

        <DialogFooter className="gap-2 sm:gap-2">
          {isEdit && event && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => icsMut.mutate(event.id)}
              disabled={icsMut.isPending}
              data-testid="download-ics-button"
            >
              <Download className="size-3.5 mr-1.5" />
              {icsMut.isPending ? 'Preparing…' : 'Download .ics'}
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={handleSave}
            disabled={!title.trim() || saving}
            data-testid="event-save-button"
          >
            {saving ? 'Saving…' : isEdit ? 'Save changes' : 'Create event'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
