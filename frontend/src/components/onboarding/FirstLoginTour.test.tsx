// TMAIL-401: FirstLoginTour exercises the GET → 3-step flow → PATCH lifecycle.
// The tour mounts via forceOpen so we can drive the step state without
// depending on TanStack Query timing. The PATCH mutation is asserted by
// stubbing the preferences API.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const fetchFirstLoginTourSeen = vi.fn();
const markFirstLoginTourSeen = vi.fn();

vi.mock('../../api/preferences', () => ({
  fetchFirstLoginTourSeen: (...args: unknown[]) => fetchFirstLoginTourSeen(...args),
  markFirstLoginTourSeen: (...args: unknown[]) => markFirstLoginTourSeen(...args),
}));

import { FirstLoginTour } from './FirstLoginTour';

function wrap(children: React.ReactNode) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('FirstLoginTour', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    markFirstLoginTourSeen.mockResolvedValue({ seen: true });
    fetchFirstLoginTourSeen.mockResolvedValue({ seen: false });
  });

  it('renders nothing while the flag is loading', () => {
    fetchFirstLoginTourSeen.mockReturnValue(new Promise(() => {}));
    render(wrap(<FirstLoginTour />));
    expect(screen.queryByTestId('first-login-tour')).not.toBeInTheDocument();
  });

  it('renders the first step when forceOpen is true', () => {
    render(wrap(<FirstLoginTour forceOpen />));
    expect(screen.getByTestId('first-login-tour')).toBeInTheDocument();
    expect(screen.getByText('Compose mail')).toBeInTheDocument();
    expect(screen.getByText('Step 1 of 3')).toBeInTheDocument();
  });

  it('advances through Compose → Inbox → Settings then PATCHes on Got it', async () => {
    render(wrap(<FirstLoginTour forceOpen />));

    // Step 1
    expect(screen.getByText('Compose mail')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('first-login-tour-next'));

    // Step 2
    expect(screen.getByText('Your inbox')).toBeInTheDocument();
    expect(screen.getByText('Step 2 of 3')).toBeInTheDocument();
    fireEvent.click(screen.getByTestId('first-login-tour-next'));

    // Step 3 — last step shows "Got it" and triggers PATCH on click.
    expect(screen.getByText('Everything else')).toBeInTheDocument();
    expect(screen.getByText('Step 3 of 3')).toBeInTheDocument();
    expect(screen.getByTestId('first-login-tour-next')).toHaveTextContent('Got it');

    fireEvent.click(screen.getByTestId('first-login-tour-next'));

    await waitFor(() => expect(markFirstLoginTourSeen).toHaveBeenCalledTimes(1));
  });

  it('PATCHes immediately when Skip is clicked', async () => {
    render(wrap(<FirstLoginTour forceOpen />));
    fireEvent.click(screen.getByTestId('first-login-tour-skip'));
    await waitFor(() => expect(markFirstLoginTourSeen).toHaveBeenCalledTimes(1));
  });

  // TMAIL-405: backdrop is no longer interactive (pointer-events: none) and
  // has no onClick handler. It exists purely as visual dimming. The popover
  // buttons (Skip / Next / Got it) are the only dismiss surfaces. Verify the
  // backdrop renders but has no click handler attached.
  it('renders the backdrop as a non-interactive visual layer', () => {
    render(wrap(<FirstLoginTour forceOpen />));
    const backdrop = screen.getByTestId('first-login-tour-backdrop');
    expect(backdrop).toBeInTheDocument();
    expect(backdrop).toHaveAttribute('aria-hidden', 'true');
    // No onClick — clicking the backdrop must not dismiss the tour.
    fireEvent.click(backdrop);
    expect(markFirstLoginTourSeen).not.toHaveBeenCalled();
  });

  it('does not render when the backend flag already says seen=true', async () => {
    fetchFirstLoginTourSeen.mockResolvedValue({ seen: true });
    render(wrap(<FirstLoginTour />));
    // Give the query a tick to resolve.
    await waitFor(() => expect(fetchFirstLoginTourSeen).toHaveBeenCalled());
    expect(screen.queryByTestId('first-login-tour')).not.toBeInTheDocument();
  });
});
