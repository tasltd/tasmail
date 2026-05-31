// TMAIL-351: attendee chip input with free-busy column. Emails are added
// on Enter/comma/blur; each chip shows a live availability indicator
// looked up via /api/calendar/free-busy. External attendees (no matching
// mailbox) get a "?" marker so the user knows their calendar wasn't
// consulted.
import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { X, AlertCircle, Check, HelpCircle } from 'lucide-react';
import { getFreeBusy, type AttendeeBusy } from '@/api/calendar';

interface AttendeesFieldProps {
  attendees: string[];
  onChange: (next: string[]) => void;
  /** Date range used for the free-busy lookup. Required so the indicator
   *  reflects availability for the event slot the user is editing. */
  rangeStart: string;
  rangeEnd: string;
  disabled?: boolean;
}

function isValidEmail(value: string): boolean {
  // Cheap RFC-ish check; the backend re-validates on insert.
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
}

interface FreeBusyDotProps {
  status: 'loading' | 'free' | 'busy' | 'not_resolved' | 'invalid';
}

function FreeBusyDot({ status }: FreeBusyDotProps) {
  if (status === 'loading') {
    return <span className="size-2 rounded-full bg-zinc-300 dark:bg-zinc-700 animate-pulse" />;
  }
  if (status === 'free') {
    return <Check className="size-3 text-green-600" aria-label="Free" />;
  }
  if (status === 'busy') {
    return <AlertCircle className="size-3 text-amber-500" aria-label="Busy" />;
  }
  if (status === 'invalid') {
    return <AlertCircle className="size-3 text-red-500" aria-label="Invalid email" />;
  }
  return <HelpCircle className="size-3 text-zinc-400" aria-label="External attendee — availability unknown" />;
}

export function AttendeesField({
  attendees,
  onChange,
  rangeStart,
  rangeEnd,
  disabled,
}: AttendeesFieldProps) {
  const [draft, setDraft] = useState('');

  const validAttendees = useMemo(
    () => attendees.filter(isValidEmail),
    [attendees],
  );

  // Free-busy lookup keyed by attendee set + window. The query is skipped
  // when there are no valid attendees so the form doesn't spam the API
  // every keystroke.
  const fbQuery = useQuery({
    queryKey: ['calendar', 'free-busy', validAttendees, rangeStart, rangeEnd],
    queryFn: () =>
      getFreeBusy({
        attendees: validAttendees,
        range_start: rangeStart,
        range_end: rangeEnd,
      }),
    enabled: validAttendees.length > 0 && Boolean(rangeStart) && Boolean(rangeEnd),
    staleTime: 30_000,
  });

  const busyByEmail = useMemo(() => {
    const map = new Map<string, AttendeeBusy>();
    fbQuery.data?.attendees.forEach((a) => {
      map.set(a.email.toLowerCase(), a);
    });
    return map;
  }, [fbQuery.data]);

  function commit(value: string) {
    const trimmed = value.trim().replace(/[,;]$/, '');
    if (!trimmed) return;
    if (attendees.some((e) => e.toLowerCase() === trimmed.toLowerCase())) {
      setDraft('');
      return;
    }
    onChange([...attendees, trimmed]);
    setDraft('');
  }

  function removeAt(index: number) {
    onChange(attendees.filter((_, i) => i !== index));
  }

  function statusFor(email: string): 'loading' | 'free' | 'busy' | 'not_resolved' | 'invalid' {
    if (!isValidEmail(email)) return 'invalid';
    if (validAttendees.length === 0) return 'not_resolved';
    if (fbQuery.isLoading || fbQuery.isFetching) return 'loading';
    const ab = busyByEmail.get(email.toLowerCase());
    if (!ab) return 'not_resolved';
    if (ab.status === 'not_resolved') return 'not_resolved';
    // Resolved + any busy interval that overlaps the requested window → busy.
    // The backend already constrains busy spans to the window we asked for,
    // so a non-empty array means they're busy at some point inside the slot.
    return ab.busy.length > 0 ? 'busy' : 'free';
  }

  // Reset draft when the parent clears the list (e.g. closing the dialog).
  useEffect(() => {
    if (attendees.length === 0) setDraft('');
  }, [attendees.length]);

  return (
    <div className="space-y-1.5">
      <label className="text-xs text-zinc-500 block">Attendees</label>
      <div
        className={`min-h-[2.25rem] flex flex-wrap gap-1.5 p-1.5 rounded-lg border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-800 ${
          disabled ? 'opacity-60' : ''
        }`}
      >
        {attendees.map((email, idx) => {
          const status = statusFor(email);
          return (
            <span
              key={`${email}-${idx}`}
              data-testid="attendee-chip"
              data-status={status}
              className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-zinc-100 dark:bg-zinc-700 text-xs"
            >
              <FreeBusyDot status={status} />
              <span className="truncate max-w-[12rem]">{email}</span>
              {!disabled && (
                <button
                  type="button"
                  onClick={() => removeAt(idx)}
                  aria-label={`Remove ${email}`}
                  className="text-zinc-400 hover:text-red-500"
                >
                  <X className="size-3" />
                </button>
              )}
            </span>
          );
        })}
        <input
          type="email"
          value={draft}
          disabled={disabled}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ',' || e.key === ';') {
              e.preventDefault();
              commit(draft);
            } else if (e.key === 'Backspace' && draft === '' && attendees.length > 0) {
              removeAt(attendees.length - 1);
            }
          }}
          onBlur={() => commit(draft)}
          placeholder={attendees.length === 0 ? 'name@example.com' : ''}
          className="flex-1 min-w-[8rem] bg-transparent text-sm outline-none px-1"
        />
      </div>
      {fbQuery.isError && (
        <p className="text-[11px] text-amber-600">
          Couldn't load attendee availability — chips will show as unknown.
        </p>
      )}
      {validAttendees.length > 0 && !fbQuery.isError && (
        <p className="text-[11px] text-zinc-400">
          Free-busy lookup checks the requested time window. Check = free,
          warning = busy, "?" = external attendee.
        </p>
      )}
    </div>
  );
}
