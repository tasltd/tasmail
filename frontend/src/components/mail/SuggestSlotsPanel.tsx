// Added (TMAIL-127): Suggest-slots panel for the Schedule Meeting modal.
//
// Given the meeting's current attendee list and duration, hits the backend's
// `/api/calendar/suggest-slots` and renders the candidate windows as a
// clickable list. Picking a slot calls `onPick(start, end)` so the parent
// modal can fill its datetime inputs.
//
// External attendees (those without a tasmail mailbox) are returned by the
// API under `unresolved_attendees`; we surface them as a yellow caveat so
// the user knows their availability is unknown rather than confirmed-free.
import { useState } from 'react';
import { Search } from 'lucide-react';
import { suggestSlots, type SuggestedSlot } from '../../api/calendar';

interface SuggestSlotsPanelProps {
  /// Current attendee list from the modal (already deduped/normalized).
  attendees: string[];
  /// Default meeting duration in minutes. Used as the initial value of the
  /// duration input; the user can override it before searching.
  defaultDurationMinutes: number;
  /// Callback fired when the user picks a slot. The parent fills its own
  /// datetime-local inputs with these values.
  onPick: (start: Date, end: Date) => void;
}

/// Format a Date as a short, locale-aware label for the slot pill.
function formatSlotLabel(start: Date, end: Date): string {
  const date = start.toLocaleDateString(undefined, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  });
  const timeOpts: Intl.DateTimeFormatOptions = { hour: 'numeric', minute: '2-digit' };
  const startLabel = start.toLocaleTimeString(undefined, timeOpts);
  const endLabel = end.toLocaleTimeString(undefined, timeOpts);
  return `${date} · ${startLabel} – ${endLabel}`;
}

export function SuggestSlotsPanel({
  attendees,
  defaultDurationMinutes,
  onPick,
}: SuggestSlotsPanelProps) {
  const [durationMinutes, setDurationMinutes] = useState(defaultDurationMinutes);
  const [daysAhead, setDaysAhead] = useState(7);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [slots, setSlots] = useState<SuggestedSlot[]>([]);
  const [unresolved, setUnresolved] = useState<string[]>([]);
  const [searched, setSearched] = useState(false);

  const handleSearch = async () => {
    setError('');
    if (attendees.length === 0) {
      setError('Add at least one attendee before searching');
      return;
    }
    if (durationMinutes <= 0) {
      setError('Duration must be a positive number');
      return;
    }
    if (daysAhead <= 0 || daysAhead > 14) {
      setError('Range must be between 1 and 14 days');
      return;
    }
    setLoading(true);
    try {
      const now = new Date();
      const rangeEnd = new Date(now.getTime() + daysAhead * 24 * 60 * 60 * 1000);
      const resp = await suggestSlots({
        attendees,
        duration_minutes: durationMinutes,
        range_start: now.toISOString(),
        range_end: rangeEnd.toISOString(),
        max_slots: 8,
        step_minutes: 30,
      });
      setSlots(resp.slots);
      setUnresolved(resp.unresolved_attendees);
      setSearched(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not fetch suggestions');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="suggest-slots-panel"
      style={{
        border: '1px solid var(--color-border, #d0d7de)',
        borderRadius: '6px',
        padding: '10px 12px',
        background: 'var(--color-bg-subtle, #f6f8fa)',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
        <Search size={14} />
        <strong style={{ fontSize: '13px' }}>Suggest a time</strong>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', alignItems: 'flex-end' }}>
        <label style={{ fontSize: '12px', display: 'flex', flexDirection: 'column' }}>
          <span style={{ marginBottom: '2px' }}>Duration (min)</span>
          <input
            type="number"
            min={5}
            max={480}
            step={5}
            value={durationMinutes}
            onChange={(e) => setDurationMinutes(Number(e.target.value))}
            aria-label="Meeting duration in minutes"
            style={{ width: '80px', padding: '4px 6px', border: '1px solid var(--color-border)', borderRadius: '4px' }}
          />
        </label>
        <label style={{ fontSize: '12px', display: 'flex', flexDirection: 'column' }}>
          <span style={{ marginBottom: '2px' }}>Within (days)</span>
          <input
            type="number"
            min={1}
            max={14}
            value={daysAhead}
            onChange={(e) => setDaysAhead(Number(e.target.value))}
            aria-label="Days ahead to search"
            style={{ width: '70px', padding: '4px 6px', border: '1px solid var(--color-border)', borderRadius: '4px' }}
          />
        </label>
        <button
          type="button"
          className="btn btn--sm"
          onClick={handleSearch}
          disabled={loading || attendees.length === 0}
          aria-label="Find available meeting slots"
        >
          {loading ? 'Searching...' : 'Find times'}
        </button>
      </div>

      {error && (
        <div role="alert" style={{ color: 'var(--color-danger, #d93025)', marginTop: '8px', fontSize: '12px' }}>
          {error}
        </div>
      )}

      {unresolved.length > 0 && (
        <div
          role="status"
          style={{
            marginTop: '8px',
            padding: '6px 8px',
            background: 'var(--color-warning-bg, #fff8c5)',
            borderLeft: '3px solid var(--color-warning, #d4a72c)',
            fontSize: '12px',
            borderRadius: '3px',
          }}
        >
          External attendees (availability unknown): {unresolved.join(', ')}
        </div>
      )}

      {searched && !loading && slots.length === 0 && !error && (
        <div style={{ marginTop: '8px', fontSize: '12px', color: 'var(--color-text-muted, #57606a)' }}>
          No common free slots in the next {daysAhead} day(s). Try a shorter meeting or a longer range.
        </div>
      )}

      {slots.length > 0 && (
        <ul
          aria-label="Suggested slots"
          style={{
            listStyle: 'none',
            padding: 0,
            margin: '8px 0 0',
            display: 'flex',
            flexDirection: 'column',
            gap: '4px',
          }}
        >
          {slots.map((slot) => {
            const start = new Date(slot.start);
            const end = new Date(slot.end);
            return (
              <li key={slot.start}>
                <button
                  type="button"
                  onClick={() => onPick(start, end)}
                  style={{
                    width: '100%',
                    textAlign: 'left',
                    padding: '6px 10px',
                    background: 'var(--color-bg-elevated, #fff)',
                    border: '1px solid var(--color-border)',
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontSize: '12px',
                  }}
                >
                  {formatSlotLabel(start, end)}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
