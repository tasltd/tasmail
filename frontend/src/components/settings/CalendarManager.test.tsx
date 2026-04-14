// Added: Tests for CalendarManager component (TMAIL-127)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CalendarManager } from './CalendarManager';

const mockListEvents = vi.fn();
const mockCreateEvent = vi.fn();
const mockCancelEvent = vi.fn();
const mockGetEvent = vi.fn();
const mockRsvpEvent = vi.fn();
const mockDownloadEventIcs = vi.fn();

vi.mock('../../api/calendar', () => ({
  listEvents: (...args: unknown[]) => mockListEvents(...args),
  createEvent: (...args: unknown[]) => mockCreateEvent(...args),
  cancelEvent: (...args: unknown[]) => mockCancelEvent(...args),
  getEvent: (...args: unknown[]) => mockGetEvent(...args),
  rsvpEvent: (...args: unknown[]) => mockRsvpEvent(...args),
  downloadEventIcs: (...args: unknown[]) => mockDownloadEventIcs(...args),
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('CalendarManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and New Event button', async () => {
    mockListEvents.mockResolvedValue([]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Calendar')).toBeInTheDocument();
    });
    expect(screen.getByText('New Event')).toBeInTheDocument();
  });

  it('shows empty state message when no events', async () => {
    mockListEvents.mockResolvedValue([]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No events yet. Create one to get started.')).toBeInTheDocument();
    });
  });

  it('renders event list with titles and status badges', async () => {
    mockListEvents.mockResolvedValue([
      { id: '1', title: 'Team Standup', status: 'confirmed', start_time: '2026-04-20T10:00:00Z', end_time: '2026-04-20T10:30:00Z', location: null, all_day: false },
      { id: '2', title: 'Sprint Review', status: 'tentative', start_time: '2026-04-21T14:00:00Z', end_time: '2026-04-21T15:00:00Z', location: 'Zoom', all_day: false },
    ]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Team Standup')).toBeInTheDocument();
    });
    expect(screen.getByText('Sprint Review')).toBeInTheDocument();
    expect(screen.getByText('confirmed')).toBeInTheDocument();
    expect(screen.getByText('tentative')).toBeInTheDocument();
  });

  it('shows create event form when New Event is clicked', async () => {
    mockListEvents.mockResolvedValue([]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Event')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Event'));
    expect(screen.getByText('New Event', { selector: 'h3' })).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Event title')).toBeInTheDocument();
  });

  it('renders datetime inputs in create form', async () => {
    mockListEvents.mockResolvedValue([]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Event')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Event'));
    expect(screen.getByText('Start')).toBeInTheDocument();
    expect(screen.getByText('End')).toBeInTheDocument();
    expect(screen.getByText('All Day')).toBeInTheDocument();
  });

  it('renders attendee input in create form', async () => {
    mockListEvents.mockResolvedValue([]);
    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('New Event')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('New Event'));
    expect(screen.getByText('Attendees')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('attendee@example.com')).toBeInTheDocument();
  });

  it('shows event detail with RSVP buttons when event is clicked', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-1', title: 'Planning', status: 'tentative', start_time: '2026-04-20T10:00:00Z', end_time: '2026-04-20T11:00:00Z', location: 'Room A', all_day: false },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-1',
      title: 'Planning',
      description: 'Sprint planning session',
      status: 'tentative',
      start_time: '2026-04-20T10:00:00Z',
      end_time: '2026-04-20T11:00:00Z',
      location: 'Room A',
      all_day: false,
      attendees: [
        { id: 'att-1', email: 'alice@example.com', display_name: 'Alice', rsvp: 'accepted', responded_at: null },
      ],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Planning')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Planning'));

    await waitFor(() => {
      expect(screen.getByText('Accept')).toBeInTheDocument();
    });
    expect(screen.getByText('Decline')).toBeInTheDocument();
    expect(screen.getByText('Maybe')).toBeInTheDocument();
  });

  it('shows ICS download button in event detail', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-2', title: 'Sync', status: 'confirmed', start_time: '2026-04-22T09:00:00Z', end_time: '2026-04-22T09:30:00Z', location: null, all_day: false },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-2',
      title: 'Sync',
      description: null,
      status: 'confirmed',
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      location: null,
      all_day: false,
      attendees: [],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Sync')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Sync'));

    await waitFor(() => {
      expect(screen.getByText('ICS')).toBeInTheDocument();
    });
  });
});
