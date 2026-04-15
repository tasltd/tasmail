// Added: Tests for CalendarView visual calendar grid component (TMAIL-118)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CalendarView } from './CalendarView';

const mockListEvents = vi.fn();

vi.mock('../../api/calendar', () => ({
  listEvents: (...args: unknown[]) => mockListEvents(...args),
}));

// Added: Helper to create test wrapper with fresh QueryClient
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

// Added: Helper to render CalendarView with default props
function renderCalendarView() {
  return render(
    <CalendarView onSelectEvent={mockOnSelectEvent} onCreateEvent={mockOnCreateEvent} />,
    { wrapper: createWrapper() },
  );
}

describe('CalendarView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListEvents.mockResolvedValue([]);
  });

  it('renders month grid with weekday headers and day cells', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('month-grid')).toBeInTheDocument();
    });
    // Added: Verify all 7 weekday headers are rendered
    expect(screen.getByText('Sun')).toBeInTheDocument();
    expect(screen.getByText('Mon')).toBeInTheDocument();
    expect(screen.getByText('Sat')).toBeInTheDocument();
    // Added: Verify day cells exist (at least 28 for any month)
    const cells = screen.getAllByTestId('month-day-cell');
    expect(cells.length).toBeGreaterThanOrEqual(28);
    // Added: Total cells should be divisible by 7
    expect(cells.length % 7).toBe(0);
  });

  it('switches to week view with 7 column headers', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('view-week')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('view-week'));

    await waitFor(() => {
      expect(screen.getByTestId('week-grid')).toBeInTheDocument();
    });
    const columnHeaders = screen.getAllByTestId('week-column-header');
    expect(columnHeaders).toHaveLength(7);
  });

  it('switches to day view with hourly time slots', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('view-day')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('view-day'));

    await waitFor(() => {
      expect(screen.getByTestId('day-grid')).toBeInTheDocument();
    });
    // Added: Day view should show 24 time slot rows
    const timeRows = screen.getAllByTestId('day-time-row');
    expect(timeRows).toHaveLength(24);
  });

  it('view switch buttons update the active view', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('month-grid')).toBeInTheDocument();
    });

    // Added: Switch to week
    fireEvent.click(screen.getByTestId('view-week'));
    await waitFor(() => {
      expect(screen.getByTestId('week-grid')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('month-grid')).not.toBeInTheDocument();

    // Added: Switch to day
    fireEvent.click(screen.getByTestId('view-day'));
    await waitFor(() => {
      expect(screen.getByTestId('day-grid')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('week-grid')).not.toBeInTheDocument();

    // Added: Switch back to month
    fireEvent.click(screen.getByTestId('view-month'));
    await waitFor(() => {
      expect(screen.getByTestId('month-grid')).toBeInTheDocument();
    });
  });

  it('navigation prev/next/today buttons update displayed date', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('calendar-heading')).toBeInTheDocument();
    });

    const now = new Date();
    const currentMonthYear = now.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
    expect(screen.getByTestId('calendar-heading').textContent).toBe(currentMonthYear);

    // Added: Click next to go to next month
    fireEvent.click(screen.getByLabelText('Next'));
    // NOTE: After clicking next, the heading text changes synchronously via state
    const nextMonth = new Date(now.getFullYear(), now.getMonth() + 1, 1);
    const nextMonthYear = nextMonth.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
    await waitFor(() => {
      expect(screen.getByTestId('calendar-heading').textContent).toBe(nextMonthYear);
    });

    // Added: Click Today to go back to current month
    fireEvent.click(screen.getByText('Today'));
    await waitFor(() => {
      expect(screen.getByTestId('calendar-heading').textContent).toBe(currentMonthYear);
    });
  });

  it('displays events on correct dates in month view', async () => {
    // Added: Create an event for today's date so it appears in the current month
    const now = new Date();
    const eventDate = new Date(now.getFullYear(), now.getMonth(), 15, 10, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-1',
        title: 'Team Standup',
        status: 'confirmed',
        start_time: eventDate.toISOString(),
        end_time: new Date(eventDate.getTime() + 3600000).toISOString(),
        location: null,
        all_day: false,
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByText('Team Standup')).toBeInTheDocument();
    });

    const chips = screen.getAllByTestId('event-chip');
    expect(chips).toHaveLength(1);
    expect(chips[0]).toHaveTextContent('Team Standup');
  });

  it('clicking on a day cell triggers onCreateEvent callback', async () => {
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('month-grid')).toBeInTheDocument();
    });

    const cells = screen.getAllByTestId('month-day-cell');
    fireEvent.click(cells[0]);
    expect(mockOnCreateEvent).toHaveBeenCalledTimes(1);
    // Added: The callback should receive a Date object
    expect(mockOnCreateEvent.mock.calls[0][0]).toBeInstanceOf(Date);
  });

  it('clicking on an event chip triggers onSelectEvent with event id', async () => {
    const now = new Date();
    const eventDate = new Date(now.getFullYear(), now.getMonth(), 15, 10, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-click',
        title: 'Click Test',
        status: 'tentative',
        start_time: eventDate.toISOString(),
        end_time: new Date(eventDate.getTime() + 3600000).toISOString(),
        location: null,
        all_day: false,
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByText('Click Test')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Click Test'));
    expect(mockOnSelectEvent).toHaveBeenCalledWith('evt-click');
  });

  it('shows empty state when no events in current period', async () => {
    mockListEvents.mockResolvedValue([]);
    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('empty-state')).toBeInTheDocument();
    });
    expect(screen.getByText('No events in this period.')).toBeInTheDocument();
  });

  it('renders events in week view time slots', async () => {
    const now = new Date();
    const eventDate = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 14, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-week',
        title: 'Week Event',
        status: 'confirmed',
        start_time: eventDate.toISOString(),
        end_time: new Date(eventDate.getTime() + 3600000).toISOString(),
        location: null,
        all_day: false,
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('view-week')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('view-week'));

    await waitFor(() => {
      expect(screen.getByTestId('week-grid')).toBeInTheDocument();
    });

    // Added: The event should appear somewhere in the week grid
    await waitFor(() => {
      expect(screen.getByText('Week Event')).toBeInTheDocument();
    });
  });

  it('renders events in day view time slots', async () => {
    const now = new Date();
    const eventDate = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 9, 0);
    mockListEvents.mockResolvedValue([
      {
        id: 'evt-day',
        title: 'Day Event',
        status: 'tentative',
        start_time: eventDate.toISOString(),
        end_time: new Date(eventDate.getTime() + 3600000).toISOString(),
        location: null,
        all_day: false,
      },
    ]);

    renderCalendarView();

    await waitFor(() => {
      expect(screen.getByTestId('view-day')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('view-day'));

    await waitFor(() => {
      expect(screen.getByTestId('day-grid')).toBeInTheDocument();
    });

    await waitFor(() => {
      expect(screen.getByText('Day Event')).toBeInTheDocument();
    });
  });
});
