// TMAIL-351: data-driven RRULE registry. Adding a new repeat option is a
// one-line entry in `RECURRENCE_PRESETS` — no code change in the form.
// The presets cover the 95th-percentile use cases (daily, weekly, biweekly,
// monthly, yearly) per RFC 5545 §3.8.5; "custom" lets the user paste a
// raw RRULE for the long tail.

export interface RecurrencePreset {
  /** Stable id used in the <select>. "none" + "custom" are reserved. */
  value: string;
  /** Human label shown in the <select>. */
  label: string;
  /** RFC 5545 RRULE body. `null` for "none". `undefined` for "custom"
   *  (the form switches to a free-text input). */
  rrule: string | null | undefined;
}

export const RECURRENCE_PRESETS: RecurrencePreset[] = [
  { value: 'none', label: 'Does not repeat', rrule: null },
  { value: 'daily', label: 'Daily', rrule: 'FREQ=DAILY' },
  { value: 'weekly', label: 'Weekly', rrule: 'FREQ=WEEKLY' },
  { value: 'biweekly', label: 'Every 2 weeks', rrule: 'FREQ=WEEKLY;INTERVAL=2' },
  { value: 'monthly', label: 'Monthly', rrule: 'FREQ=MONTHLY' },
  { value: 'yearly', label: 'Yearly', rrule: 'FREQ=YEARLY' },
  { value: 'custom', label: 'Custom (RRULE…)', rrule: undefined },
];

/**
 * PURPOSE: Reverse-map an RRULE body to a preset value so the edit form can
 * pre-select the right option. Unknown rules fall through to "custom" so
 * the user sees the raw RRULE and can keep editing.
 */
export function presetForRrule(rrule: string | null | undefined): string {
  if (!rrule) return 'none';
  const match = RECURRENCE_PRESETS.find((p) => p.rrule && p.rrule === rrule);
  return match ? match.value : 'custom';
}

/**
 * PURPOSE: Resolve a preset value + the user-typed custom RRULE into the
 * value that goes on the wire. Returns `null` to clear an existing rule.
 */
export function resolveRrule(presetValue: string, customRrule: string): string | null {
  if (presetValue === 'none') return null;
  if (presetValue === 'custom') {
    const trimmed = customRrule.trim();
    return trimmed.length > 0 ? trimmed : null;
  }
  const preset = RECURRENCE_PRESETS.find((p) => p.value === presetValue);
  return preset?.rrule ?? null;
}
