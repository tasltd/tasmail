// Added (TMAIL-127): tests for the suggest-slots panel embedded in the
// Schedule Meeting modal. Covers the happy path, error handling, the empty
// state, the validation guards, and the external-attendee warning.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SuggestSlotsPanel } from './SuggestSlotsPanel';
import * as calendarApi from '../../api/calendar';

vi.mock('../../api/calendar', async () => {
  const actual = await vi.importActual<typeof import('../../api/calendar')>('../../api/calendar');
  return {
    ...actual,
    suggestSlots: vi.fn(),
  };
});

const mockedSuggest = vi.mocked(calendarApi.suggestSlots);

describe('SuggestSlotsPanel', () => {
  beforeEach(() => {
    mockedSuggest.mockReset();
  });

  it('renders the search controls with default duration', () => {
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com']}
        defaultDurationMinutes={45}
        onPick={() => {}}
      />,
    );
    const durationInput = screen.getByLabelText(/duration/i) as HTMLInputElement;
    expect(durationInput.value).toBe('45');
    expect(screen.getByRole('button', { name: /find available meeting slots/i })).toBeTruthy();
  });

  it('warns when there are no attendees', async () => {
    render(
      <SuggestSlotsPanel attendees={[]} defaultDurationMinutes={30} onPick={() => {}} />,
    );
    const button = screen.getByRole('button', { name: /find available meeting slots/i });
    // Without attendees the button is disabled; force-click via fireEvent.
    expect((button as HTMLButtonElement).disabled).toBe(true);
  });

  it('calls suggestSlots with the current attendees and duration', async () => {
    mockedSuggest.mockResolvedValueOnce({
      slots: [
        { start: '2026-06-01T09:00:00Z', end: '2026-06-01T09:30:00Z' },
        { start: '2026-06-01T10:00:00Z', end: '2026-06-01T10:30:00Z' },
      ],
      unresolved_attendees: [],
    });
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com', 'bob@example.com']}
        defaultDurationMinutes={30}
        onPick={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => expect(mockedSuggest).toHaveBeenCalledTimes(1));
    const arg = mockedSuggest.mock.calls[0][0];
    expect(arg.attendees).toEqual(['alice@example.com', 'bob@example.com']);
    expect(arg.duration_minutes).toBe(30);
    expect(arg.max_slots).toBe(8);
  });

  it('renders the returned slot list and fires onPick when one is selected', async () => {
    mockedSuggest.mockResolvedValueOnce({
      slots: [{ start: '2026-06-01T09:00:00Z', end: '2026-06-01T09:30:00Z' }],
      unresolved_attendees: [],
    });
    const onPick = vi.fn();
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com']}
        defaultDurationMinutes={30}
        onPick={onPick}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => screen.getByRole('list', { name: /suggested slots/i }));
    const slotButton = screen.getAllByRole('button').find((b) => b.textContent && b.textContent.includes('9:00'));
    expect(slotButton).toBeTruthy();
    fireEvent.click(slotButton!);
    expect(onPick).toHaveBeenCalledTimes(1);
    const [start, end] = onPick.mock.calls[0];
    expect(start.toISOString()).toBe('2026-06-01T09:00:00.000Z');
    expect(end.toISOString()).toBe('2026-06-01T09:30:00.000Z');
  });

  it('surfaces external attendees as a warning', async () => {
    mockedSuggest.mockResolvedValueOnce({
      slots: [],
      unresolved_attendees: ['gmail-user@gmail.com'],
    });
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com', 'gmail-user@gmail.com']}
        defaultDurationMinutes={30}
        onPick={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => screen.getByText(/availability unknown/i));
    expect(screen.getByText(/gmail-user@gmail.com/)).toBeTruthy();
  });

  it('shows an empty-state hint when the backend returns zero slots', async () => {
    mockedSuggest.mockResolvedValueOnce({ slots: [], unresolved_attendees: [] });
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com']}
        defaultDurationMinutes={30}
        onPick={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => screen.getByText(/no common free slots/i));
  });

  it('surfaces backend errors as an alert', async () => {
    mockedSuggest.mockRejectedValueOnce(new Error('Backend down'));
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com']}
        defaultDurationMinutes={30}
        onPick={() => {}}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toContain('Backend down');
    });
  });

  it('rejects an out-of-range search horizon', async () => {
    render(
      <SuggestSlotsPanel
        attendees={['alice@example.com']}
        defaultDurationMinutes={30}
        onPick={() => {}}
      />,
    );
    const daysInput = screen.getByLabelText(/days ahead/i) as HTMLInputElement;
    fireEvent.change(daysInput, { target: { value: '30' } });
    fireEvent.click(screen.getByRole('button', { name: /find available meeting slots/i }));
    await waitFor(() => {
      const alert = screen.getByRole('alert');
      expect(alert.textContent).toMatch(/1 and 14/);
    });
    expect(mockedSuggest).not.toHaveBeenCalled();
  });
});
