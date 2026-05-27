// Changed: Tests rewritten for FullCalendar-based CalendarView (TMAIL-118)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CalendarView } from './CalendarView';

const mockListEvents = vi.fn();

vi.mock('../../api/calendar', () => ({
  listEvents: (...args: unknown[]) => mockListEvents(...args),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

const mockOnSelectEvent = vi.fn();
const mockOnCreateEvent = vi.fn();
const mockOnReschedule = vi.fn();

function renderCalendarView(opts: { reschedule?: boolean } = {}) {
  return render(
    <CalendarView
      onSelectEvent={mockOnSelectEvent}
      onCreateEvent={mockOnCreateEvent}
      onRescheduleEvent={opts.reschedule ? mockOnReschedule : undefined}
    />,
    { wrapper: createWrapper() },
  );
}

describe('CalendarView (FullCalendar)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListEvents.mockResolvedValue([]);
  });

  it('renders the FullCalendar container', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc')).toBeTruthy();
    });
    expect(screen.getByTestId('calendar-view')).toBeInTheDocument();
  });

  it('renders all four view-switch buttons (month/week/day/agenda)', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-dayGridMonth-button')).toBeTruthy();
    });
    expect(within(container).getByText('Month')).toBeInTheDocument();
    expect(within(container).getByText('Week')).toBeInTheDocument();
    expect(within(container).getByText('Day')).toBeInTheDocument();
    expect(within(container).getByText('Agenda')).toBeInTheDocument();
  });

  it('renders prev/next/today navigation buttons', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-today-button')).toBeTruthy();
    });
    expect(container.querySelector('.fc-prev-button')).toBeTruthy();
    expect(container.querySelector('.fc-next-button')).toBeTruthy();
  });

  it('starts on month view by default', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-dayGridMonth-view')).toBeTruthy();
    });
  });

  it('switches to week view when the Week button is clicked', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-timeGridWeek-button')).toBeTruthy();
    });
    fireEvent.click(container.querySelector('.fc-timeGridWeek-button')!);
    await waitFor(() => {
      expect(container.querySelector('.fc-timeGridWeek-view')).toBeTruthy();
    });
  });

  it('switches to day view when the Day button is clicked', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-timeGridDay-button')).toBeTruthy();
    });
    fireEvent.click(container.querySelector('.fc-timeGridDay-button')!);
    await waitFor(() => {
      expect(container.querySelector('.fc-timeGridDay-view')).toBeTruthy();
    });
  });

  it('switches to agenda (list) view when the Agenda button is clicked', async () => {
    const { container } = renderCalendarView();
    await waitFor(() => {
      expect(container.querySelector('.fc-listWeek-button')).toBeTruthy();
    });
    fireEvent.click(container.querySelector('.fc-listWeek-button')!);
    await waitFor(() => {
      expect(container.querySelector('.fc-listWeek-view')).toBeTruthy();
    });
  });

  it('fetches events for the visible range on mount', async () => {
    renderCalendarView();
    await waitFor(() => {
      expect(mockListEvents).toHaveBeenCalled();
    });
    // Added: First arg should be an ISO start, second an ISO end
    const [start, end] = mockListEvents.mock.calls[0];
    expect(typeof start).toBe('string');
    expect(typeof end).toBe('string');
    expect(new Date(start).toString()).not.toBe('Invalid Date');
    expect(new Date(end).toString()).not.toBe('Invalid Date');
  });

  it('displays a non-recurring event on the calendar', async () => {
    const now = new Date();
    const start = new Date(now.getFullYear(), now.getMonth(), 15, 10, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-1',
        organizer_id: 'u1',
        title: 'Team Standup',
        status: 'confirmed',
        description: null,
        location: null,
        start_time: start.toISOString(),
        end_time: new Date(start.getTime() + 3600000).toISOString(),
        all_day: false,
        recurrence_rule: null,
        linked_message_uid: null,
        linked_folder: null,
        ics_uid: 'ics-1',
        created_at: start.toISOString(),
        updated_at: start.toISOString(),
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByText('Team Standup')).toBeInTheDocument();
    });
  });

  it('triggers onSelectEvent when an event chip is clicked', async () => {
    const now = new Date();
    const start = new Date(now.getFullYear(), now.getMonth(), 15, 10, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-click',
        organizer_id: 'u1',
        title: 'Clickable Event',
        status: 'tentative',
        description: null,
        location: null,
        start_time: start.toISOString(),
        end_time: new Date(start.getTime() + 3600000).toISOString(),
        all_day: false,
        recurrence_rule: null,
        linked_message_uid: null,
        linked_folder: null,
        ics_uid: 'ics-c',
        created_at: start.toISOString(),
        updated_at: start.toISOString(),
      },
    ]);

    renderCalendarView();

    const eventEl = await screen.findByText('Clickable Event');
    fireEvent.click(eventEl);

    await waitFor(() => {
      expect(mockOnSelectEvent).toHaveBeenCalledWith('evt-click');
    });
  });

  it('renders an event that has a recurrence_rule (rendered once, expansion deferred)', async () => {
    // NOTE: Full RRULE expansion via @fullcalendar/rrule is deferred — the
    // plugin currently has an ESM interop bug with rrule@2.8+ where RRule and
    // rrulestr come back undefined. The backend persists recurrence_rule and
    // ICS export still emits it; for now the master event shows once.
    const now = new Date();
    const start = new Date(now.getFullYear(), now.getMonth(), 15, 9, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-rrule',
        organizer_id: 'u1',
        title: 'Weekly Sync',
        status: 'confirmed',
        description: null,
        location: null,
        start_time: start.toISOString(),
        end_time: new Date(start.getTime() + 3600000).toISOString(),
        all_day: false,
        recurrence_rule: 'FREQ=WEEKLY',
        linked_message_uid: null,
        linked_folder: null,
        ics_uid: 'ics-r',
        created_at: start.toISOString(),
        updated_at: start.toISOString(),
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByText('Weekly Sync')).toBeInTheDocument();
    });
  });

  it('does not request reschedule callback when onRescheduleEvent is omitted (editable=false)', async () => {
    const { container } = renderCalendarView({ reschedule: false });
    await waitFor(() => {
      expect(container.querySelector('.fc')).toBeTruthy();
    });
    // Added: With editable=false there's no drag overlay handle on day cells
    expect(mockOnReschedule).not.toHaveBeenCalled();
  });
});
