// Added: Tests for calendar API module (TMAIL-127)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  listEvents,
  createEvent,
  getEvent,
  updateEvent,
  cancelEvent,
  rsvpEvent,
  downloadEventIcs,
} from './calendar';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('calendar API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listEvents', () => {
    it('calls GET /calendar/events without params', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listEvents();
      expect(apiClient.get).toHaveBeenCalledWith('/calendar/events');
    });

    it('calls GET /calendar/events with date range params', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listEvents('2026-04-01T00:00:00Z', '2026-04-30T23:59:59Z');
      expect(apiClient.get).toHaveBeenCalledWith(
        '/calendar/events?start=2026-04-01T00%3A00%3A00Z&end=2026-04-30T23%3A59%3A59Z'
      );
    });
  });

  describe('createEvent', () => {
    it('calls POST /calendar/events with event data', async () => {
      const eventData = {
        title: 'Team Meeting',
        start_time: '2026-04-20T10:00:00Z',
        end_time: '2026-04-20T11:00:00Z',
        attendees: [{ email: 'alice@example.com' }],
      };
      const mockResponse = { id: '1', ...eventData, attendees: [] };
      vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

      const result = await createEvent(eventData);
      expect(apiClient.post).toHaveBeenCalledWith('/calendar/events', eventData);
      expect(result.id).toBe('1');
    });
  });

  describe('getEvent', () => {
    it('calls GET /calendar/events/:id', async () => {
      vi.mocked(apiClient.get).mockResolvedValue({ id: 'abc', title: 'Sync', attendees: [] });
      const result = await getEvent('abc');
      expect(apiClient.get).toHaveBeenCalledWith('/calendar/events/abc');
      expect(result.title).toBe('Sync');
    });
  });

  describe('updateEvent', () => {
    it('calls PUT /calendar/events/:id with update data', async () => {
      const updateData = { title: 'Updated Meeting', status: 'confirmed' };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'abc', ...updateData });

      await updateEvent('abc', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/calendar/events/abc', updateData);
    });
  });

  describe('cancelEvent', () => {
    it('calls DELETE /calendar/events/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await cancelEvent('abc');
      expect(apiClient.delete).toHaveBeenCalledWith('/calendar/events/abc');
    });
  });

  describe('rsvpEvent', () => {
    it('calls POST /calendar/events/:id/rsvp with status', async () => {
      vi.mocked(apiClient.post).mockResolvedValue({ id: '1', rsvp: 'accepted' });
      const result = await rsvpEvent('abc', { status: 'accepted' });
      expect(apiClient.post).toHaveBeenCalledWith('/calendar/events/abc/rsvp', { status: 'accepted' });
      expect(result.rsvp).toBe('accepted');
    });
  });

  describe('downloadEventIcs', () => {
    it('calls GET /calendar/events/:id/ics', async () => {
      vi.mocked(apiClient.get).mockResolvedValue('BEGIN:VCALENDAR...');
      const result = await downloadEventIcs('abc');
      expect(apiClient.get).toHaveBeenCalledWith('/calendar/events/abc/ics');
      expect(result).toContain('BEGIN:VCALENDAR');
    });
  });
});
