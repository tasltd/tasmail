import { describe, it, expect } from 'vitest';
import { formatMessageDate, formatFullDate, formatRelativeDate } from './date';

describe('date utils', () => {
  describe('formatMessageDate', () => {
    it('returns empty string for null', () => {
      expect(formatMessageDate(null)).toBe('');
    });

    it('returns original string for invalid date', () => {
      expect(formatMessageDate('not-a-date')).toBe('not-a-date');
    });

    it('formats today as HH:mm', () => {
      const today = new Date();
      today.setHours(14, 30, 0, 0);
      const result = formatMessageDate(today.toISOString());
      expect(result).toBe('14:30');
    });

    it('formats yesterday as "Yesterday"', () => {
      const yesterday = new Date();
      yesterday.setDate(yesterday.getDate() - 1);
      yesterday.setHours(10, 0, 0, 0);
      expect(formatMessageDate(yesterday.toISOString())).toBe('Yesterday');
    });

    it('formats dates in current year as "MMM d"', () => {
      const date = new Date();
      date.setMonth(0, 15); // Jan 15
      date.setDate(date.getDate() - 10); // ensure it's not today/yesterday
      // Only test if it's still this year and not yesterday/today
      const result = formatMessageDate(new Date(date.getFullYear(), 0, 2).toISOString());
      expect(result).toMatch(/Jan 2/);
    });

    it('formats older dates with year', () => {
      const result = formatMessageDate('2020-06-15T10:00:00Z');
      expect(result).toMatch(/Jun 15, 2020/);
    });
  });

  describe('formatFullDate', () => {
    it('returns empty string for null', () => {
      expect(formatFullDate(null)).toBe('');
    });

    it('returns original string for invalid date', () => {
      expect(formatFullDate('invalid')).toBe('invalid');
    });

    it('formats a valid date with day name and time', () => {
      const result = formatFullDate('2024-03-15T14:30:00Z');
      // Should contain day name, month name, day, year, and time
      expect(result).toMatch(/Friday/);
      expect(result).toMatch(/March/);
      expect(result).toMatch(/15/);
      expect(result).toMatch(/2024/);
    });
  });

  describe('formatRelativeDate', () => {
    it('returns relative time string', () => {
      const recent = new Date(Date.now() - 60000).toISOString(); // 1 min ago
      const result = formatRelativeDate(recent);
      expect(result).toMatch(/ago/);
    });

    it('returns original string for invalid input', () => {
      expect(formatRelativeDate('not-valid')).toBe('not-valid');
    });
  });
});
