// Added: Schedule Meeting modal triggered from the Composer (TMAIL-127)
import { useState, useEffect, useMemo, type FormEvent } from 'react';
import { X, Plus } from 'lucide-react';
import { createEvent } from '../../api/calendar';
import type { CalendarEventWithAttendees } from '../../api/calendar';
// Added (TMAIL-127): suggest-slots panel — uses the new /api/calendar/free-busy
// + /api/calendar/suggest-slots endpoints to recommend common free windows.
import { SuggestSlotsPanel } from './SuggestSlotsPanel';

interface ScheduleMeetingModalProps {
  initialTitle: string;
  initialAttendees: string[];
  onClose: () => void;
  onCreated?: (event: CalendarEventWithAttendees) => void;
}

// Added: Returns a datetime-local-formatted string for the next round half hour
function defaultStart(): string {
  const d = new Date();
  d.setMinutes(d.getMinutes() < 30 ? 30 : 60, 0, 0);
  return toLocalInputValue(d);
}

function defaultEnd(): string {
  const d = new Date();
  d.setMinutes(d.getMinutes() < 30 ? 30 : 60, 0, 0);
  d.setMinutes(d.getMinutes() + 30);
  return toLocalInputValue(d);
}

// Added: Format a Date as the value expected by <input type="datetime-local">
function toLocalInputValue(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

// Added: Dedupes and trims the initial attendee list pulled from To/Cc fields
function normalizeAttendees(raw: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const entry of raw) {
    const trimmed = entry.trim();
    if (!trimmed) continue;
    if (seen.has(trimmed.toLowerCase())) continue;
    seen.add(trimmed.toLowerCase());
    out.push(trimmed);
  }
  return out;
}

export function ScheduleMeetingModal({
  initialTitle,
  initialAttendees,
  onClose,
  onCreated,
}: ScheduleMeetingModalProps) {
  const initialList = useMemo(() => normalizeAttendees(initialAttendees), [initialAttendees]);
  const [title, setTitle] = useState(initialTitle);
  const [description, setDescription] = useState('');
  const [location, setLocation] = useState('');
  const [startTime, setStartTime] = useState(defaultStart);
  const [endTime, setEndTime] = useState(defaultEnd);
  const [attendees, setAttendees] = useState<string[]>(initialList);
  const [attendeeInput, setAttendeeInput] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  // Added (TMAIL-127): collapsed by default to keep the modal tight; expands
  // when the user wants to lean on the slot suggester.
  const [showSuggest, setShowSuggest] = useState(false);

  // Added (TMAIL-127): callback wired into <SuggestSlotsPanel onPick>. When
  // the user clicks a candidate slot we fill the datetime-local inputs (which
  // expect the value in the user's local timezone, not ISO/UTC).
  const handlePickSlot = (start: Date, end: Date) => {
    setStartTime(toLocalInputValue(start));
    setEndTime(toLocalInputValue(end));
  };

  // Added (TMAIL-127): derived duration in minutes, fed to SuggestSlotsPanel
  // so its default matches whatever the user has currently set in the modal.
  const currentDurationMinutes = useMemo(() => {
    const s = new Date(startTime);
    const e = new Date(endTime);
    if (Number.isNaN(s.getTime()) || Number.isNaN(e.getTime()) || e <= s) {
      return 30;
    }
    return Math.max(5, Math.round((e.getTime() - s.getTime()) / 60000));
  }, [startTime, endTime]);

  // Added: Close on Escape key
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onClose();
    }
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  const handleAddAttendee = () => {
    const trimmed = attendeeInput.trim();
    if (!trimmed) return;
    if (attendees.some((a) => a.toLowerCase() === trimmed.toLowerCase())) {
      setAttendeeInput('');
      return;
    }
    setAttendees([...attendees, trimmed]);
    setAttendeeInput('');
  };

  const handleRemoveAttendee = (email: string) => {
    setAttendees(attendees.filter((a) => a !== email));
  };

  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setError('');

    if (!title.trim()) {
      setError('Title is required');
      return;
    }
    const start = new Date(startTime);
    const end = new Date(endTime);
    if (Number.isNaN(start.getTime()) || Number.isNaN(end.getTime())) {
      setError('Start and end time are required');
      return;
    }
    if (end <= start) {
      setError('End time must be after start time');
      return;
    }

    setSubmitting(true);
    try {
      const event = await createEvent({
        title: title.trim(),
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        start_time: start.toISOString(),
        end_time: end.toISOString(),
        attendees: attendees.length > 0 ? attendees.map((email) => ({ email })) : undefined,
      });
      onCreated?.(event);
      onClose();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create event');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      className="schedule-meeting-modal__overlay"
      onClick={onClose}
      role="presentation"
      style={{
        position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
        display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1100,
      }}
    >
      <div
        role="dialog"
        aria-label="Schedule meeting"
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'var(--color-bg-elevated, #fff)', borderRadius: '8px', padding: '20px',
          width: 'min(520px, 92vw)', maxHeight: '90vh', overflowY: 'auto',
          boxShadow: '0 10px 40px rgba(0,0,0,0.25)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', marginBottom: '12px' }}>
          <h3 style={{ flex: 1, margin: 0, fontSize: '16px' }}>Schedule Meeting</h3>
          <button
            type="button"
            className="btn btn--icon"
            onClick={onClose}
            aria-label="Close"
          >
            <X size={18} />
          </button>
        </div>

        {error && (
          <div role="alert" style={{ color: 'var(--color-danger, #d93025)', marginBottom: '8px', fontSize: '13px' }}>
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <div>
            <label htmlFor="meeting-title" style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>Title</label>
            <input
              id="meeting-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Event title"
              required
              style={{ width: '100%', padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px' }}
            />
          </div>

          <div style={{ display: 'flex', gap: '8px' }}>
            <div style={{ flex: 1 }}>
              <label htmlFor="meeting-start" style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>Start</label>
              <input
                id="meeting-start"
                type="datetime-local"
                value={startTime}
                onChange={(e) => setStartTime(e.target.value)}
                required
                style={{ width: '100%', padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px' }}
              />
            </div>
            <div style={{ flex: 1 }}>
              <label htmlFor="meeting-end" style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>End</label>
              <input
                id="meeting-end"
                type="datetime-local"
                value={endTime}
                onChange={(e) => setEndTime(e.target.value)}
                required
                style={{ width: '100%', padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px' }}
              />
            </div>
          </div>

          {/* Added (TMAIL-127): collapsible suggest-times helper. Hidden by
              default so the modal stays compact; expands on demand. */}
          <div>
            <button
              type="button"
              onClick={() => setShowSuggest((v) => !v)}
              aria-expanded={showSuggest}
              aria-controls="suggest-slots-panel"
              style={{
                background: 'transparent',
                border: 'none',
                color: 'var(--color-primary, #4a90d9)',
                cursor: 'pointer',
                padding: 0,
                fontSize: '12px',
              }}
            >
              {showSuggest ? 'Hide suggestions' : 'Suggest times based on availability'}
            </button>
            {showSuggest && (
              <div id="suggest-slots-panel" style={{ marginTop: '8px' }}>
                <SuggestSlotsPanel
                  attendees={attendees}
                  defaultDurationMinutes={currentDurationMinutes}
                  onPick={handlePickSlot}
                />
              </div>
            )}
          </div>

          <div>
            <label htmlFor="meeting-location" style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>Location</label>
            <input
              id="meeting-location"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
              placeholder="Optional location or meeting link"
              style={{ width: '100%', padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px' }}
            />
          </div>

          <div>
            <label htmlFor="meeting-description" style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>Description</label>
            <textarea
              id="meeting-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Optional description"
              rows={3}
              style={{ width: '100%', padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px', resize: 'vertical' }}
            />
          </div>

          <div>
            <label style={{ fontSize: '13px', display: 'block', marginBottom: '4px' }}>Attendees</label>
            <div style={{ display: 'flex', gap: '6px' }}>
              <input
                value={attendeeInput}
                onChange={(e) => setAttendeeInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleAddAttendee();
                  }
                }}
                placeholder="attendee@example.com"
                aria-label="Add attendee"
                style={{ flex: 1, padding: '6px 10px', border: '1px solid var(--color-border)', borderRadius: '6px' }}
              />
              <button type="button" className="btn" onClick={handleAddAttendee}>
                <Plus size={14} /> Add
              </button>
            </div>
            {attendees.length > 0 && (
              <ul aria-label="Attendees" style={{ listStyle: 'none', padding: 0, margin: '8px 0 0', display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                {attendees.map((email) => (
                  <li
                    key={email}
                    style={{
                      display: 'inline-flex', alignItems: 'center', gap: '4px',
                      padding: '4px 8px', borderRadius: '12px',
                      background: 'var(--color-bg-subtle, #eef1f5)', fontSize: '12px',
                    }}
                  >
                    {email}
                    <button
                      type="button"
                      onClick={() => handleRemoveAttendee(email)}
                      aria-label={`Remove ${email}`}
                      style={{
                        background: 'transparent', border: 'none', cursor: 'pointer',
                        padding: 0, display: 'inline-flex', alignItems: 'center',
                      }}
                    >
                      <X size={12} />
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px', marginTop: '4px' }}>
            <button type="button" className="btn" onClick={onClose} disabled={submitting}>
              Cancel
            </button>
            <button type="submit" className="btn btn--primary" disabled={submitting}>
              {submitting ? 'Creating...' : 'Create Event'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
