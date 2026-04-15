// Added: Visual calendar view component with month/week/day grids (TMAIL-118)
import { useState, useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, Calendar } from 'lucide-react';
import { listEvents } from '../../api/calendar';
import type { CalendarEvent } from '../../api/calendar';
import {
  getDaysInMonth,
  getWeekDays,
  getHoursOfDay,
  isSameDay,
  formatMonthYear,
  formatWeekdayShort,
  getHourFromIso,
} from '../../utils/calendar-helpers';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Calendar view mode type for grid switching
type CalViewMode = 'month' | 'week' | 'day';

// Added: Day header labels for the grid
const WEEKDAY_HEADERS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

// Added: Event dot/chip color based on status
const EVENT_CHIP_COLORS: Record<string, string> = {
  confirmed: '#22c55e',
  tentative: '#f59e0b',
  cancelled: '#ef4444',
};

interface CalendarViewProps {
  onSelectEvent: (eventId: string) => void;
  onCreateEvent: (date?: Date) => void;
}

// PURPOSE: Visual calendar grid with month/week/day views and event display
export function CalendarView({ onSelectEvent, onCreateEvent }: CalendarViewProps) {
  const [currentDate, setCurrentDate] = useState(() => new Date());
  const [calView, setCalView] = useState<CalViewMode>('month');

  // Added: Compute date range for API query based on current view
  const dateRange = useMemo(() => {
    if (calView === 'month') {
      const start = new Date(currentDate.getFullYear(), currentDate.getMonth(), 1);
      const end = new Date(currentDate.getFullYear(), currentDate.getMonth() + 1, 0, 23, 59, 59);
      return { start: start.toISOString(), end: end.toISOString() };
    }
    if (calView === 'week') {
      const days = getWeekDays(currentDate);
      const end = new Date(days[6]);
      end.setHours(23, 59, 59);
      return { start: days[0].toISOString(), end: end.toISOString() };
    }
    // Added: Day view range
    const start = new Date(currentDate.getFullYear(), currentDate.getMonth(), currentDate.getDate());
    const end = new Date(currentDate.getFullYear(), currentDate.getMonth(), currentDate.getDate(), 23, 59, 59);
    return { start: start.toISOString(), end: end.toISOString() };
  }, [currentDate, calView]);

  const { data: events, isLoading } = useQuery({
    queryKey: ['calendar-events-view', dateRange.start, dateRange.end],
    queryFn: () => listEvents(dateRange.start, dateRange.end),
  });

  // Added: Navigate to previous period (month/week/day)
  const handlePrev = () => {
    setCurrentDate((prev) => {
      if (calView === 'month') return new Date(prev.getFullYear(), prev.getMonth() - 1, 1);
      if (calView === 'week') return new Date(prev.getFullYear(), prev.getMonth(), prev.getDate() - 7);
      return new Date(prev.getFullYear(), prev.getMonth(), prev.getDate() - 1);
    });
  };

  // Added: Navigate to next period (month/week/day)
  const handleNext = () => {
    setCurrentDate((prev) => {
      if (calView === 'month') return new Date(prev.getFullYear(), prev.getMonth() + 1, 1);
      if (calView === 'week') return new Date(prev.getFullYear(), prev.getMonth(), prev.getDate() + 7);
      return new Date(prev.getFullYear(), prev.getMonth(), prev.getDate() + 1);
    });
  };

  // Added: Jump to today
  const handleToday = () => setCurrentDate(new Date());

  // Added: Get events for a specific day
  const getEventsForDay = (day: Date): CalendarEvent[] => {
    if (!events) return [];
    return events.filter((evt) => isSameDay(new Date(evt.start_time), day));
  };

  // Added: Get events for a specific day and hour
  const getEventsForHour = (day: Date, hour: number): CalendarEvent[] => {
    if (!events) return [];
    return events.filter((evt) => {
      const evtDate = new Date(evt.start_time);
      return isSameDay(evtDate, day) && getHourFromIso(evt.start_time) === hour;
    });
  };

  const today = new Date();

  return (
    <div data-testid="calendar-view">
      {/* Added: Calendar toolbar with navigation and view switcher */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '16px', flexWrap: 'wrap' }}>
        <button className="btn btn--icon" onClick={handlePrev} aria-label="Previous">
          <ChevronLeft size={20} />
        </button>
        <button className="btn" onClick={handleToday}>Today</button>
        <button className="btn btn--icon" onClick={handleNext} aria-label="Next">
          <ChevronRight size={20} />
        </button>
        <h3 style={{ flex: 1, fontSize: '16px', margin: 0 }} data-testid="calendar-heading">
          {calView === 'month' && formatMonthYear(currentDate)}
          {calView === 'week' && (() => {
            const days = getWeekDays(currentDate);
            return `${days[0].toLocaleDateString('en-US', { month: 'short', day: 'numeric' })} – ${days[6].toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })}`;
          })()}
          {calView === 'day' && currentDate.toLocaleDateString('en-US', { weekday: 'long', month: 'long', day: 'numeric', year: 'numeric' })}
        </h3>
        <div style={{ display: 'flex', gap: '4px' }}>
          <button
            className={`btn ${calView === 'month' ? 'btn--primary' : ''}`}
            onClick={() => setCalView('month')}
            data-testid="view-month"
          >
            Month
          </button>
          <button
            className={`btn ${calView === 'week' ? 'btn--primary' : ''}`}
            onClick={() => setCalView('week')}
            data-testid="view-week"
          >
            Week
          </button>
          <button
            className={`btn ${calView === 'day' ? 'btn--primary' : ''}`}
            onClick={() => setCalView('day')}
            data-testid="view-day"
          >
            Day
          </button>
        </div>
      </div>

      {/* Added: Show loading skeleton while events are fetching */}
      {isLoading && <LoadingSkeleton rows={6} />}

      {/* Added: Month view grid */}
      {!isLoading && calView === 'month' && (
        <div data-testid="month-grid">
          {/* Added: Weekday headers */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)', gap: '1px', background: 'var(--color-border)' }}>
            {WEEKDAY_HEADERS.map((header) => (
              <div
                key={header}
                style={{
                  background: 'var(--color-bg-secondary, #f8f9fa)',
                  padding: '6px 4px',
                  textAlign: 'center',
                  fontSize: '12px',
                  fontWeight: 600,
                }}
              >
                {header}
              </div>
            ))}
          </div>
          {/* Added: Day cells */}
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)', gap: '1px', background: 'var(--color-border)' }}>
            {getDaysInMonth(currentDate.getFullYear(), currentDate.getMonth()).map((day, idx) => {
              const isCurrentMonth = day.getMonth() === currentDate.getMonth();
              const isToday = isSameDay(day, today);
              const dayEvents = getEventsForDay(day);
              return (
                <div
                  key={idx}
                  data-testid="month-day-cell"
                  onClick={() => onCreateEvent(day)}
                  style={{
                    background: isToday ? 'var(--color-primary-light, #e8f0fe)' : 'var(--color-bg, white)',
                    padding: '4px',
                    minHeight: '70px',
                    cursor: 'pointer',
                    opacity: isCurrentMonth ? 1 : 0.4,
                  }}
                >
                  <div style={{ fontSize: '12px', fontWeight: isToday ? 700 : 400, marginBottom: '2px' }}>
                    {day.getDate()}
                  </div>
                  {dayEvents.slice(0, 3).map((evt) => (
                    <div
                      key={evt.id}
                      data-testid="event-chip"
                      onClick={(e) => { e.stopPropagation(); onSelectEvent(evt.id); }}
                      style={{
                        fontSize: '11px',
                        padding: '1px 4px',
                        marginBottom: '1px',
                        borderRadius: '3px',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                        background: EVENT_CHIP_COLORS[evt.status] || EVENT_CHIP_COLORS.tentative,
                        color: 'white',
                        cursor: 'pointer',
                      }}
                      title={evt.title}
                    >
                      {evt.title}
                    </div>
                  ))}
                  {dayEvents.length > 3 && (
                    <div style={{ fontSize: '10px', color: 'var(--color-text-secondary)' }}>
                      +{dayEvents.length - 3} more
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Added: Week view with time slots */}
      {!isLoading && calView === 'week' && (() => {
        const weekDays = getWeekDays(currentDate);
        const hours = getHoursOfDay();
        return (
          <div data-testid="week-grid" style={{ overflowX: 'auto' }}>
            {/* Added: Column headers (weekdays) */}
            <div style={{ display: 'grid', gridTemplateColumns: '60px repeat(7, 1fr)', gap: '1px', background: 'var(--color-border)' }}>
              <div style={{ background: 'var(--color-bg-secondary, #f8f9fa)', padding: '6px 4px' }} />
              {weekDays.map((day, i) => (
                <div
                  key={i}
                  data-testid="week-column-header"
                  style={{
                    background: isSameDay(day, today) ? 'var(--color-primary-light, #e8f0fe)' : 'var(--color-bg-secondary, #f8f9fa)',
                    padding: '6px 4px',
                    textAlign: 'center',
                    fontSize: '12px',
                    fontWeight: 600,
                  }}
                >
                  {formatWeekdayShort(day)} {day.getDate()}
                </div>
              ))}
            </div>
            {/* Added: Time slot rows */}
            <div style={{ maxHeight: '500px', overflowY: 'auto' }}>
              {hours.map((hourLabel, hIdx) => (
                <div
                  key={hourLabel}
                  style={{ display: 'grid', gridTemplateColumns: '60px repeat(7, 1fr)', gap: '1px', background: 'var(--color-border)' }}
                >
                  <div
                    data-testid="time-slot-label"
                    style={{
                      background: 'var(--color-bg-secondary, #f8f9fa)',
                      padding: '4px',
                      fontSize: '11px',
                      color: 'var(--color-text-secondary)',
                      textAlign: 'right',
                    }}
                  >
                    {hourLabel}
                  </div>
                  {weekDays.map((day, dIdx) => {
                    const hourEvents = getEventsForHour(day, hIdx);
                    return (
                      <div
                        key={dIdx}
                        data-testid="week-time-cell"
                        onClick={() => onCreateEvent(day)}
                        style={{
                          background: 'var(--color-bg, white)',
                          padding: '2px',
                          minHeight: '28px',
                          cursor: 'pointer',
                        }}
                      >
                        {hourEvents.map((evt) => (
                          <div
                            key={evt.id}
                            data-testid="event-chip"
                            onClick={(e) => { e.stopPropagation(); onSelectEvent(evt.id); }}
                            style={{
                              fontSize: '10px',
                              padding: '1px 3px',
                              borderRadius: '3px',
                              background: EVENT_CHIP_COLORS[evt.status] || EVENT_CHIP_COLORS.tentative,
                              color: 'white',
                              cursor: 'pointer',
                              overflow: 'hidden',
                              textOverflow: 'ellipsis',
                              whiteSpace: 'nowrap',
                            }}
                            title={evt.title}
                          >
                            {evt.title}
                          </div>
                        ))}
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          </div>
        );
      })()}

      {/* Added: Day view with hourly time slots */}
      {!isLoading && calView === 'day' && (() => {
        const hours = getHoursOfDay();
        return (
          <div data-testid="day-grid">
            {hours.map((hourLabel, hIdx) => {
              const hourEvents = getEventsForHour(currentDate, hIdx);
              return (
                <div
                  key={hourLabel}
                  data-testid="day-time-row"
                  onClick={() => onCreateEvent(currentDate)}
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '60px 1fr',
                    gap: '1px',
                    borderBottom: '1px solid var(--color-border)',
                    cursor: 'pointer',
                  }}
                >
                  <div
                    data-testid="time-slot-label"
                    style={{
                      padding: '6px 4px',
                      fontSize: '12px',
                      color: 'var(--color-text-secondary)',
                      textAlign: 'right',
                    }}
                  >
                    {hourLabel}
                  </div>
                  <div style={{ padding: '4px', minHeight: '36px' }}>
                    {hourEvents.map((evt) => (
                      <div
                        key={evt.id}
                        data-testid="event-chip"
                        onClick={(e) => { e.stopPropagation(); onSelectEvent(evt.id); }}
                        style={{
                          fontSize: '12px',
                          padding: '2px 6px',
                          marginBottom: '2px',
                          borderRadius: '4px',
                          background: EVENT_CHIP_COLORS[evt.status] || EVENT_CHIP_COLORS.tentative,
                          color: 'white',
                          cursor: 'pointer',
                        }}
                        title={evt.title}
                      >
                        <Calendar size={12} style={{ verticalAlign: 'middle', marginRight: '4px' }} />
                        {evt.title}
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        );
      })()}

      {/* Added: Empty state when no events in current view */}
      {!isLoading && events && events.length === 0 && (
        <p
          data-testid="empty-state"
          style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}
        >
          No events in this period.
        </p>
      )}
    </div>
  );
}
