// Added (TMAIL-269): tests for the public BookingPage.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { BookingPage } from './BookingPage';

const mockGetPublicEvent = vi.fn();
const mockSubmitPublicRsvp = vi.fn();

vi.mock('../../api/public-calendar', async (orig) => {
  const actual = await orig<typeof import('../../api/public-calendar')>();
  return {
    ...actual,
    getPublicEvent: (...args: unknown[]) => mockGetPublicEvent(...args),
    submitPublicRsvp: (...args: unknown[]) => mockSubmitPublicRsvp(...args),
  };
});

const TOKEN = '11111111-2222-3333-4444-555555555555';

function renderAt(path: string) {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <Routes>
        <Route path="/book/:token" element={<BookingPage />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('BookingPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the event summary on successful fetch', async () => {
    mockGetPublicEvent.mockResolvedValueOnce({
      id: 'evt-1',
      title: 'Discovery Call',
      description: '30 minute intro chat',
      location: 'Zoom',
      start_time: '2026-04-20T10:00:00Z',
      end_time: '2026-04-20T10:30:00Z',
      all_day: false,
      status: 'confirmed',
    });

    renderAt(`/book/${TOKEN}`);

    expect(await screen.findByRole('heading', { name: 'Discovery Call' })).toBeInTheDocument();
    expect(screen.getByText(/Zoom/)).toBeInTheDocument();
    expect(screen.getByText('30 minute intro chat')).toBeInTheDocument();
    expect(mockGetPublicEvent).toHaveBeenCalledWith(TOKEN);
  });

  it('shows the unavailable message when the token returns 404', async () => {
    const { PublicCalendarError } = await import('../../api/public-calendar');
    mockGetPublicEvent.mockRejectedValueOnce(new PublicCalendarError(404, 'not found'));

    renderAt(`/book/${TOKEN}`);

    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/no longer active/i)).toBeInTheDocument();
  });

  it('submits the RSVP with the correct payload and shows confirmation', async () => {
    mockGetPublicEvent.mockResolvedValueOnce({
      id: 'evt-2',
      title: 'Sync',
      description: null,
      location: null,
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      all_day: false,
      status: 'confirmed',
    });
    mockSubmitPublicRsvp.mockResolvedValueOnce({
      email: 'jane@example.com',
      display_name: 'Jane',
      rsvp: 'accepted',
      responded_at: '2026-04-21T12:00:00Z',
    });

    renderAt(`/book/${TOKEN}`);

    await screen.findByRole('heading', { name: 'Sync' });

    fireEvent.change(screen.getByPlaceholderText('Jane Doe'), { target: { value: 'Jane' } });
    fireEvent.change(screen.getByPlaceholderText('you@example.com'), {
      target: { value: 'Jane@Example.COM' },
    });
    fireEvent.click(screen.getByText('Send response'));

    await waitFor(() => {
      expect(mockSubmitPublicRsvp).toHaveBeenCalledWith(TOKEN, {
        email: 'jane@example.com',
        name: 'Jane',
        status: 'accepted',
      });
    });

    expect(await screen.findByText(/Thanks for responding/)).toBeInTheDocument();
    expect(screen.getByText(/accepted/)).toBeInTheDocument();
  });

  it('rejects invalid email client-side before calling the API', async () => {
    mockGetPublicEvent.mockResolvedValueOnce({
      id: 'evt-3',
      title: 'Test',
      description: null,
      location: null,
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      all_day: false,
      status: 'confirmed',
    });

    renderAt(`/book/${TOKEN}`);
    await screen.findByRole('heading', { name: 'Test' });

    fireEvent.change(screen.getByPlaceholderText('you@example.com'), {
      target: { value: 'not-an-email' },
    });
    fireEvent.click(screen.getByText('Send response'));

    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.getByText(/valid email/i)).toBeInTheDocument();
    expect(mockSubmitPublicRsvp).not.toHaveBeenCalled();
  });

  it('lets the user pick declined or maybe', async () => {
    mockGetPublicEvent.mockResolvedValueOnce({
      id: 'evt-4',
      title: 'Pick',
      description: null,
      location: null,
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      all_day: false,
      status: 'confirmed',
    });
    mockSubmitPublicRsvp.mockResolvedValueOnce({
      email: 'bob@example.com',
      display_name: null,
      rsvp: 'declined',
      responded_at: '2026-04-21T12:00:00Z',
    });

    renderAt(`/book/${TOKEN}`);
    await screen.findByRole('heading', { name: 'Pick' });

    fireEvent.click(screen.getByLabelText('No, I cannot attend'));
    fireEvent.change(screen.getByPlaceholderText('you@example.com'), {
      target: { value: 'bob@example.com' },
    });
    fireEvent.click(screen.getByText('Send response'));

    await waitFor(() => {
      expect(mockSubmitPublicRsvp).toHaveBeenCalledWith(
        TOKEN,
        expect.objectContaining({ status: 'declined' }),
      );
    });
  });
});
