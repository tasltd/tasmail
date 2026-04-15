// Added: Tests for calendar utility functions (TMAIL-118)
import { describe, it, expect } from 'vitest';
import {
  getDaysInMonth,
  getWeekDays,
  getHoursOfDay,
  isSameDay,
  formatMonthYear,
  formatWeekdayShort,
  getHourFromIso,
} from './calendar-helpers';

describe('getDaysInMonth', () => {
  it('returns correct number of days for April 2026 (30 days)', () => {
    const days = getDaysInMonth(2026, 3); // NOTE: month is 0-based, 3 = April
    // Added: April 2026 starts on Wednesday (day 3), so 3 padding days before + 30 days = 33, padded to 35
    expect(days.length % 7).toBe(0);
    // Added: Verify that April 1st is present in the grid
    const april1 = days.find(
      (d) => d.getFullYear() === 2026 && d.getMonth() === 3 && d.getDate() === 1
    );
    expect(april1).toBeDefined();
  });

  it('returns correct number of days for February in a leap year', () => {
    const days = getDaysInMonth(2024, 1); // NOTE: Feb 2024 is a leap year
    expect(days.length % 7).toBe(0);
    // Added: Feb 29 should exist
    const feb29 = days.find(
      (d) => d.getFullYear() === 2024 && d.getMonth() === 1 && d.getDate() === 29
    );
    expect(feb29).toBeDefined();
  });

  it('includes padding days from previous month when month does not start on Sunday', () => {
    // Added: April 2026 starts on Wednesday, so Sunday/Monday/Tuesday should be from March
    const days = getDaysInMonth(2026, 3);
    const firstDay = days[0];
    expect(firstDay.getDay()).toBe(0); // Sunday
  });

  it('pads to a complete last row', () => {
    const days = getDaysInMonth(2026, 3);
    expect(days.length % 7).toBe(0);
  });
});

describe('getWeekDays', () => {
  it('returns 7 days', () => {
    const days = getWeekDays(new Date(2026, 3, 15)); // Wednesday April 15, 2026
    expect(days).toHaveLength(7);
  });

  it('starts on Sunday and ends on Saturday', () => {
    const days = getWeekDays(new Date(2026, 3, 15));
    expect(days[0].getDay()).toBe(0); // Sunday
    expect(days[6].getDay()).toBe(6); // Saturday
  });

  it('contains the input date', () => {
    const inputDate = new Date(2026, 3, 15);
    const days = getWeekDays(inputDate);
    const found = days.some((d) => isSameDay(d, inputDate));
    expect(found).toBe(true);
  });
});

describe('getHoursOfDay', () => {
  it('returns 24 hour labels', () => {
    const hours = getHoursOfDay();
    expect(hours).toHaveLength(24);
  });

  it('starts with 00:00 and ends with 23:00', () => {
    const hours = getHoursOfDay();
    expect(hours[0]).toBe('00:00');
    expect(hours[23]).toBe('23:00');
  });
});

describe('isSameDay', () => {
  it('returns true for same calendar day', () => {
    const a = new Date(2026, 3, 15, 10, 30);
    const b = new Date(2026, 3, 15, 22, 0);
    expect(isSameDay(a, b)).toBe(true);
  });

  it('returns false for different days', () => {
    const a = new Date(2026, 3, 15);
    const b = new Date(2026, 3, 16);
    expect(isSameDay(a, b)).toBe(false);
  });

  it('returns false for same day in different months', () => {
    const a = new Date(2026, 3, 15);
    const b = new Date(2026, 4, 15);
    expect(isSameDay(a, b)).toBe(false);
  });
});

describe('formatMonthYear', () => {
  it('formats April 2026 correctly', () => {
    expect(formatMonthYear(new Date(2026, 3, 1))).toBe('April 2026');
  });

  it('formats January 2025 correctly', () => {
    expect(formatMonthYear(new Date(2025, 0, 15))).toBe('January 2025');
  });
});

describe('formatWeekdayShort', () => {
  it('returns short weekday name', () => {
    // Added: April 15, 2026 is a Wednesday
    const result = formatWeekdayShort(new Date(2026, 3, 15));
    expect(result).toBe('Wed');
  });
});

describe('getHourFromIso', () => {
  it('extracts hour from ISO string', () => {
    // NOTE: This depends on local timezone, so we build the ISO from a local Date
    const date = new Date(2026, 3, 15, 14, 30);
    const hour = getHourFromIso(date.toISOString());
    expect(hour).toBe(14);
  });
});
