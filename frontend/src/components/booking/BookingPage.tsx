// Added (TMAIL-269 / TMAIL-127): public booking page for external participants.
//
// Mounted at /book/:token by App.tsx. The page is intentionally minimal —
// no app shell, no sidebar, no login requirement — so anyone with the share
// link can see the event details and RSVP.

import { useEffect, useState, type FormEvent } from 'react';
import { useParams, Link } from 'react-router-dom';
import {
  getPublicEvent,
  submitPublicRsvp,
  PublicCalendarError,
  type PublicEventSummary,
  type PublicRsvpStatus,
} from '../../api/public-calendar';
import './BookingPage.css';

// Render the start/end times in the visitor's local zone so they don't have to
// do timezone math. Hours/minutes only when both endpoints are on the same day.
function formatRange(startIso: string, endIso: string, allDay: boolean): string {
  const start = new Date(startIso);
  const end = new Date(endIso);
  if (allDay) {
    return start.toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' });
  }
  const sameDay = start.toDateString() === end.toDateString();
  const datePart = start.toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' });
  const timeOpts: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit' };
  if (sameDay) {
    return `${datePart} · ${start.toLocaleTimeString(undefined, timeOpts)} – ${end.toLocaleTimeString(undefined, timeOpts)}`;
  }
  return `${datePart} ${start.toLocaleTimeString(undefined, timeOpts)} → ${end.toLocaleDateString(undefined, { month: 'long', day: 'numeric' })} ${end.toLocaleTimeString(undefined, timeOpts)}`;
}

export function BookingPage() {
  const { token } = useParams<{ token: string }>();
  const [event, setEvent] = useState<PublicEventSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Form state
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<PublicRsvpStatus>('accepted');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState<{ status: string } | null>(null);

  useEffect(() => {
    if (!token) {
      setLoadError('Missing booking token.');
      setLoading(false);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const e = await getPublicEvent(token);
        if (!cancelled) setEvent(e);
      } catch (err) {
        if (cancelled) return;
        if (err instanceof PublicCalendarError && err.status === 404) {
          setLoadError('This booking link is no longer active.');
        } else {
          setLoadError('Could not load the event. Please check the link or try again later.');
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [token]);

  async function handleSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setSubmitError(null);

    const trimmedEmail = email.trim().toLowerCase();
    if (!trimmedEmail || !trimmedEmail.includes('@')) {
      setSubmitError('Please enter a valid email address.');
      return;
    }

    if (!token) {
      setSubmitError('Missing booking token.');
      return;
    }

    setSubmitting(true);
    try {
      const result = await submitPublicRsvp(token, {
        email: trimmedEmail,
        name: name.trim() || undefined,
        status,
      });
      setConfirmed({ status: result.rsvp });
    } catch (err) {
      let msg = 'Could not submit your response.';
      if (err instanceof PublicCalendarError) {
        msg = err.message || msg;
      } else if (err instanceof Error) {
        msg = err.message;
      }
      setSubmitError(msg);
    } finally {
      setSubmitting(false);
    }
  }

  if (loading) {
    return (
      <div className="booking-page" role="status" aria-busy="true">
        <div className="booking-page__card">Loading…</div>
      </div>
    );
  }

  if (loadError || !event) {
    return (
      <div className="booking-page">
        <div className="booking-page__card booking-page__card--error" role="alert">
          <h1>Booking link unavailable</h1>
          <p>{loadError ?? 'Event not found.'}</p>
          <p>
            <Link to="/">Return to TASMail</Link>
          </p>
        </div>
      </div>
    );
  }

  if (confirmed) {
    return (
      <div className="booking-page">
        <div className="booking-page__card" role="status">
          <h1>Thanks for responding</h1>
          <p>
            Your response (<strong>{confirmed.status}</strong>) was recorded for
            <strong> {event.title}</strong>.
          </p>
          <p className="booking-page__muted">
            The organizer has been notified. You can close this page.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="booking-page">
      <div className="booking-page__card">
        <header className="booking-page__header">
          <h1>{event.title}</h1>
          <p className="booking-page__when">{formatRange(event.start_time, event.end_time, event.all_day)}</p>
          {event.location && <p className="booking-page__location">📍 {event.location}</p>}
          {event.description && <p className="booking-page__description">{event.description}</p>}
        </header>

        <form className="booking-page__form" onSubmit={handleSubmit} noValidate>
          <h2>Will you attend?</h2>

          {submitError && (
            <div className="booking-page__error" role="alert">
              {submitError}
            </div>
          )}

          <label className="booking-page__field">
            <span>Your name (optional)</span>
            <input
              type="text"
              autoComplete="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              maxLength={200}
              placeholder="Jane Doe"
            />
          </label>

          <label className="booking-page__field">
            <span>Email</span>
            <input
              type="email"
              autoComplete="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="you@example.com"
            />
          </label>

          <fieldset className="booking-page__choices">
            <legend>Your response</legend>
            {(['accepted', 'maybe', 'declined'] as const).map((s) => (
              <label key={s} className={`booking-page__choice ${status === s ? 'is-active' : ''}`}>
                <input
                  type="radio"
                  name="rsvp-status"
                  value={s}
                  checked={status === s}
                  onChange={() => setStatus(s)}
                />
                <span>{s === 'accepted' ? 'Yes, I will attend' : s === 'maybe' ? 'Maybe' : 'No, I cannot attend'}</span>
              </label>
            ))}
          </fieldset>

          <button type="submit" className="booking-page__submit" disabled={submitting}>
            {submitting ? 'Submitting…' : 'Send response'}
          </button>
        </form>

        <footer className="booking-page__footer">
          <p>
            Powered by <Link to="/">TASMail</Link>
          </p>
        </footer>
      </div>
    </div>
  );
}
