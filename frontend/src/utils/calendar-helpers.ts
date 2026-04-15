// Added: Calendar utility functions for visual calendar view (TMAIL-118)

// PURPOSE: Get all dates that should appear in a month grid (including padding days from prev/next months)
export function getDaysInMonth(year: number, month: number): Date[] {
  // NOTE: month is 0-based (0 = January)
  const firstDay = new Date(year, month, 1);
  const lastDay = new Date(year, month + 1, 0);

  const days: Date[] = [];

  // Added: Pad with days from previous month so grid starts on Sunday
  const startDayOfWeek = firstDay.getDay();
  for (let i = startDayOfWeek - 1; i >= 0; i--) {
    days.push(new Date(year, month, -i));
  }

  // Added: All days of the current month
  for (let d = 1; d <= lastDay.getDate(); d++) {
    days.push(new Date(year, month, d));
  }

  // Added: Pad with days from next month to fill the last row (total should be multiple of 7)
  const remaining = 7 - (days.length % 7);
  if (remaining < 7) {
    for (let i = 1; i <= remaining; i++) {
      days.push(new Date(year, month + 1, i));
    }
  }

  return days;
}

// PURPOSE: Get the 7 dates for the week containing the given date (Sunday to Saturday)
export function getWeekDays(date: Date): Date[] {
  const dayOfWeek = date.getDay();
  const sunday = new Date(date.getFullYear(), date.getMonth(), date.getDate() - dayOfWeek);
  const days: Date[] = [];
  for (let i = 0; i < 7; i++) {
    days.push(new Date(sunday.getFullYear(), sunday.getMonth(), sunday.getDate() + i));
  }
  return days;
}

// PURPOSE: Get array of hour labels from 00:00 to 23:00 for day/week time slot views
export function getHoursOfDay(): string[] {
  const hours: string[] = [];
  for (let h = 0; h < 24; h++) {
    hours.push(`${h.toString().padStart(2, '0')}:00`);
  }
  return hours;
}

// PURPOSE: Check if two dates represent the same calendar day
export function isSameDay(date1: Date, date2: Date): boolean {
  return (
    date1.getFullYear() === date2.getFullYear() &&
    date1.getMonth() === date2.getMonth() &&
    date1.getDate() === date2.getDate()
  );
}

// PURPOSE: Format a date as "Month Year" string, e.g. "April 2026"
export function formatMonthYear(date: Date): string {
  return date.toLocaleDateString('en-US', { month: 'long', year: 'numeric' });
}

// PURPOSE: Format a date as a short weekday label, e.g. "Sun", "Mon"
export function formatWeekdayShort(date: Date): string {
  return date.toLocaleDateString('en-US', { weekday: 'short' });
}

// PURPOSE: Get the hour (0-23) from an ISO date string for time-slot placement
export function getHourFromIso(isoString: string): number {
  return new Date(isoString).getHours();
}
