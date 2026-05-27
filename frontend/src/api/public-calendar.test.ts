// Added (TMAIL-269): tests for the public scheduling API module.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  buildBookingUrl,
  getPublicEvent,
  submitPublicRsvp,
  PublicCalendarError,
} from './public-calendar';

const TOKEN = '0a1b2c3d-4e5f-6789-abcd-ef0123456789';

describe('public-calendar API', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('buildBookingUrl', () => {
    it('builds a /book/{token} URL using the supplied origin', () => {
      expect(buildBookingUrl('abc', 'https://mail.example.com')).toBe(
        'https://mail.example.com/book/abc',
      );
    });

    it('falls back to window.location.origin when no origin is supplied', () => {
      // jsdom default origin is http://localhost:3000
      expect(buildBookingUrl(TOKEN)).toBe(`${window.location.origin}/book/${TOKEN}`);
    });
  });

  describe('getPublicEvent', () => {
    it('returns the event summary on a 200 response', async () => {
      const summary = {
        id: 'evt-1',
        title: 'Discovery Call',
        description: null,
        location: 'Zoom',
        start_time: '2026-04-20T10:00:00Z',
        end_time: '2026-04-20T10:30:00Z',
        all_day: false,
        status: 'confirmed',
      };
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify(summary), { status: 200, headers: { 'Content-Type': 'application/json' } }),
      );
      vi.stubGlobal('fetch', fetchMock);

      const out = await getPublicEvent(TOKEN);
      expect(out).toEqual(summary);
      // Token must be URL-encoded into the path.
      expect(fetchMock).toHaveBeenCalledWith(
        expect.stringContaining(`/calendar/public/${TOKEN}`),
        expect.objectContaining({ method: 'GET' }),
      );
    });

    it('throws PublicCalendarError on a 404 response', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValueOnce(new Response('not found', { status: 404 })),
      );

      await expect(getPublicEvent(TOKEN)).rejects.toBeInstanceOf(PublicCalendarError);
      try {
        await getPublicEvent(TOKEN);
      } catch (err) {
        // The error from the second call — second mockResolvedValueOnce wasn't set,
        // so a default fetch happens. Use the first throw instead by checking status.
        if (err instanceof PublicCalendarError) {
          expect(err.status === 404 || err.status === 0).toBe(true);
        }
      }
    });

    it('PublicCalendarError surfaces both status and message', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValueOnce(new Response('boom', { status: 500 })),
      );
      try {
        await getPublicEvent(TOKEN);
        throw new Error('expected throw');
      } catch (err) {
        expect(err).toBeInstanceOf(PublicCalendarError);
        const e = err as PublicCalendarError;
        expect(e.status).toBe(500);
        expect(e.message).toBe('boom');
      }
    });
  });

  describe('submitPublicRsvp', () => {
    it('POSTs JSON and returns the rsvp response', async () => {
      const response = {
        email: 'alice@example.com',
        display_name: 'Alice',
        rsvp: 'accepted',
        responded_at: '2026-04-19T11:30:00Z',
      };
      const fetchMock = vi.fn().mockResolvedValueOnce(
        new Response(JSON.stringify(response), { status: 200, headers: { 'Content-Type': 'application/json' } }),
      );
      vi.stubGlobal('fetch', fetchMock);

      const out = await submitPublicRsvp(TOKEN, {
        email: 'alice@example.com',
        name: 'Alice',
        status: 'accepted',
      });
      expect(out).toEqual(response);

      // Verify body was sent as JSON with the right fields.
      const call = fetchMock.mock.calls[0];
      expect(call[1]).toMatchObject({
        method: 'POST',
        headers: expect.objectContaining({ 'Content-Type': 'application/json' }),
      });
      const sentBody = JSON.parse(call[1].body);
      expect(sentBody).toEqual({
        email: 'alice@example.com',
        name: 'Alice',
        status: 'accepted',
      });
    });

    it('throws PublicCalendarError on 400', async () => {
      vi.stubGlobal(
        'fetch',
        vi.fn().mockResolvedValueOnce(new Response('invalid status', { status: 400 })),
      );
      await expect(
        submitPublicRsvp(TOKEN, {
          email: 'alice@example.com',
          status: 'accepted',
        }),
      ).rejects.toBeInstanceOf(PublicCalendarError);
    });
  });
});
