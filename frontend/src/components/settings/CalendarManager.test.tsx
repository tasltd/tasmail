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
// Added (TMAIL-269): the public-booking toggle uses updateEvent under the hood,
// so the test mock has to expose it even when individual specs don't exercise it.
const mockUpdateEvent = vi.fn();

vi.mock('../../api/calendar', () => ({
  listEvents: (...args: unknown[]) => mockListEvents(...args),
  createEvent: (...args: unknown[]) => mockCreateEvent(...args),
  cancelEvent: (...args: unknown[]) => mockCancelEvent(...args),
  getEvent: (...args: unknown[]) => mockGetEvent(...args),
  rsvpEvent: (...args: unknown[]) => mockRsvpEvent(...args),
  downloadEventIcs: (...args: unknown[]) => mockDownloadEventIcs(...args),
  updateEvent: (...args: unknown[]) => mockUpdateEvent(...args),
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

  // Added (TMAIL-269): public booking link controls in EventDetail.
  it('shows public share controls in event detail, hidden URL when disabled', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-3', title: 'Demo Call', status: 'confirmed', start_time: '2026-05-01T09:00:00Z', end_time: '2026-05-01T09:30:00Z', location: null, all_day: false },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-3',
      title: 'Demo Call',
      description: null,
      status: 'confirmed',
      start_time: '2026-05-01T09:00:00Z',
      end_time: '2026-05-01T09:30:00Z',
      location: null,
      all_day: false,
      public_token: '00000000-0000-0000-0000-000000000abc',
      public_enabled: false,
      attendees: [],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByText('Demo Call')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Demo Call'));

    // Section + toggle visible; URL block hidden until toggled on.
    await waitFor(() =>
      expect(screen.getByTestId('public-share-section')).toBeInTheDocument(),
    );
    expect(screen.getByText('Public booking link')).toBeInTheDocument();
    expect(screen.queryByTestId('public-share-url')).not.toBeInTheDocument();
    expect(screen.queryByTestId('public-share-copy')).not.toBeInTheDocument();
  });

  it('shows shareable URL and copy button when public_enabled is true', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-4', title: 'Discovery', status: 'confirmed', start_time: '2026-05-02T09:00:00Z', end_time: '2026-05-02T09:30:00Z', location: null, all_day: false },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-4',
      title: 'Discovery',
      description: null,
      status: 'confirmed',
      start_time: '2026-05-02T09:00:00Z',
      end_time: '2026-05-02T09:30:00Z',
      location: null,
      all_day: false,
      public_token: 'abcd-token-1234',
      public_enabled: true,
      attendees: [],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByText('Discovery')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Discovery'));

    await waitFor(() =>
      expect(screen.getByTestId('public-share-url')).toBeInTheDocument(),
    );
    const urlEl = screen.getByTestId('public-share-url');
    expect(urlEl.textContent).toContain('/book/abcd-token-1234');
    expect(screen.getByTestId('public-share-copy')).toBeInTheDocument();
  });

  it('clicking the public toggle calls updateEvent with public_enabled flag', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-5', title: 'Pitch', status: 'confirmed', start_time: '2026-05-03T09:00:00Z', end_time: '2026-05-03T09:30:00Z', location: null, all_day: false },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-5',
      title: 'Pitch',
      description: null,
      status: 'confirmed',
      start_time: '2026-05-03T09:00:00Z',
      end_time: '2026-05-03T09:30:00Z',
      location: null,
      all_day: false,
      public_token: 'tok-pitch',
      public_enabled: false,
      attendees: [],
    });
    mockUpdateEvent.mockResolvedValue({});

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => expect(screen.getByText('Pitch')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Pitch'));

    await waitFor(() =>
      expect(screen.getByLabelText('Allow external participants to book via a share link')).toBeInTheDocument(),
    );
    fireEvent.click(screen.getByLabelText('Allow external participants to book via a share link'));

    await waitFor(() => {
      expect(mockUpdateEvent).toHaveBeenCalledWith('evt-5', { public_enabled: true });
    });
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

  // ---- TMAIL-269: public booking link controls ----

  it('renders the public booking toggle unchecked by default in event detail', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-pub-1', title: 'PrivateMeeting', status: 'tentative', start_time: '2026-04-22T09:00:00Z', end_time: '2026-04-22T09:30:00Z', location: null, all_day: false, public_enabled: false, public_token: 'tok-abc' },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-pub-1',
      title: 'PrivateMeeting',
      description: null,
      status: 'tentative',
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      location: null,
      all_day: false,
      public_enabled: false,
      public_token: 'tok-abc',
      attendees: [],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByText('PrivateMeeting')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('PrivateMeeting'));

    await waitFor(() => {
      expect(screen.getByText('Public booking link')).toBeInTheDocument();
    });
    const toggle = screen.getByLabelText(/external participants/i) as HTMLInputElement;
    expect(toggle.checked).toBe(false);
    // The share URL must NOT be visible until public_enabled is true.
    expect(screen.queryByTestId('public-share-url')).not.toBeInTheDocument();
  });

  it('shows the share URL and Copy button when public_enabled is true', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-pub-2', title: 'PublicMeeting', status: 'confirmed', start_time: '2026-04-22T09:00:00Z', end_time: '2026-04-22T09:30:00Z', location: null, all_day: false, public_enabled: true, public_token: 'tok-xyz' },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-pub-2',
      title: 'PublicMeeting',
      description: null,
      status: 'confirmed',
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      location: null,
      all_day: false,
      public_enabled: true,
      public_token: 'tok-xyz',
      attendees: [],
    });

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByText('PublicMeeting')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('PublicMeeting'));

    const shareUrl = await screen.findByTestId('public-share-url');
    expect(shareUrl.textContent).toContain('/book/tok-xyz');
    expect(screen.getByTestId('public-share-copy')).toBeInTheDocument();
  });

  it('calls updateEvent({public_enabled: true}) when the owner enables sharing', async () => {
    mockListEvents.mockResolvedValue([
      { id: 'evt-pub-3', title: 'ToEnable', status: 'tentative', start_time: '2026-04-22T09:00:00Z', end_time: '2026-04-22T09:30:00Z', location: null, all_day: false, public_enabled: false, public_token: 'tok-3' },
    ]);
    mockGetEvent.mockResolvedValue({
      id: 'evt-pub-3',
      title: 'ToEnable',
      description: null,
      status: 'tentative',
      start_time: '2026-04-22T09:00:00Z',
      end_time: '2026-04-22T09:30:00Z',
      location: null,
      all_day: false,
      public_enabled: false,
      public_token: 'tok-3',
      attendees: [],
    });
    mockUpdateEvent.mockResolvedValue({ id: 'evt-pub-3', public_enabled: true });

    render(<CalendarManager />, { wrapper: createWrapper() });
    await waitFor(() => {
      expect(screen.getByText('ToEnable')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('ToEnable'));

    const toggle = await screen.findByLabelText(/external participants/i);
    fireEvent.click(toggle);

    await waitFor(() => {
      expect(mockUpdateEvent).toHaveBeenCalledWith('evt-pub-3', { public_enabled: true });
    });
  });
});
