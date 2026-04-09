import { format, formatDistanceToNow, isToday, isYesterday, parseISO } from 'date-fns';

export function formatMessageDate(dateStr: string | null): string {
  if (!dateStr) return '';

  try {
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return dateStr;

    if (isToday(date)) {
      return format(date, 'HH:mm');
    }
    if (isYesterday(date)) {
      return 'Yesterday';
    }
    // Within the current year
    if (date.getFullYear() === new Date().getFullYear()) {
      return format(date, 'MMM d');
    }
    return format(date, 'MMM d, yyyy');
  } catch {
    return dateStr;
  }
}

export function formatFullDate(dateStr: string | null): string {
  if (!dateStr) return '';
  try {
    const date = new Date(dateStr);
    if (isNaN(date.getTime())) return dateStr;
    return format(date, "EEEE, MMMM d, yyyy 'at' HH:mm");
  } catch {
    return dateStr;
  }
}

export function formatRelativeDate(dateStr: string): string {
  try {
    return formatDistanceToNow(parseISO(dateStr), { addSuffix: true });
  } catch {
    return dateStr;
  }
}
