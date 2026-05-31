// TMAIL-351: lightweight slot suggester panel. Fires
// /api/calendar/suggest-slots with the attendee list + duration the user
// already entered in the parent form, then lets them click a slot to
// auto-fill the start/end fields.
import { useMutation } from '@tanstack/react-query';
import { Wand2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { suggestSlots, type SuggestedSlot } from '@/api/calendar';

interface SuggestSlotsPanelProps {
  attendees: string[];
  durationMinutes: number;
  /** Search window. Defaults to (now, now + 14 days) when not provided. */
  rangeStart: string;
  rangeEnd: string;
  onPick: (slot: SuggestedSlot) => void;
  disabled?: boolean;
}

function formatSlotLabel(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}

export function SuggestSlotsPanel({
  attendees,
  durationMinutes,
  rangeStart,
  rangeEnd,
  onPick,
  disabled,
}: SuggestSlotsPanelProps) {
  const suggestMut = useMutation({
    mutationFn: () =>
      suggestSlots({
        attendees,
        duration_minutes: durationMinutes,
        range_start: rangeStart,
        range_end: rangeEnd,
        max_slots: 6,
        step_minutes: 30,
      }),
  });

  const canSuggest =
    !disabled &&
    attendees.length > 0 &&
    durationMinutes > 0 &&
    Boolean(rangeStart) &&
    Boolean(rangeEnd);

  return (
    <div className="space-y-2">
      <Button
        type="button"
        variant="outline"
        size="sm"
        disabled={!canSuggest || suggestMut.isPending}
        onClick={() => suggestMut.mutate()}
        data-testid="suggest-slots-button"
      >
        <Wand2 className="size-3.5 mr-1.5" />
        {suggestMut.isPending ? 'Finding slots…' : 'Suggest slots'}
      </Button>

      {suggestMut.isError && (
        <p className="text-xs text-red-600">
          Couldn't load suggestions: {String((suggestMut.error as Error)?.message ?? 'unknown')}
        </p>
      )}

      {suggestMut.data && (
        <div className="rounded-lg border border-zinc-200 dark:border-zinc-700 p-2 space-y-1.5">
          {suggestMut.data.unresolved_attendees.length > 0 && (
            <p className="text-[11px] text-amber-600">
              Couldn't read availability for:{' '}
              {suggestMut.data.unresolved_attendees.join(', ')}. Treated as
              always-free during the search.
            </p>
          )}
          {suggestMut.data.slots.length === 0 ? (
            <p className="text-xs text-zinc-500 py-1">
              No common free windows in the next two weeks. Try a shorter
              duration or fewer attendees.
            </p>
          ) : (
            <ul className="space-y-1" data-testid="suggested-slots-list">
              {suggestMut.data.slots.map((slot) => (
                <li key={slot.start}>
                  <button
                    type="button"
                    onClick={() => onPick(slot)}
                    className="w-full text-left text-xs px-2 py-1 rounded hover:bg-blue-50 dark:hover:bg-blue-950 text-blue-700 dark:text-blue-300"
                  >
                    {formatSlotLabel(slot.start)}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
