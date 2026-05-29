import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import 'fake-indexeddb/auto';

// NOTE: Mock the api/* modules pulled in lazily by background-sync so the
// dynamic imports during processPending() don't try to hit the real network.
vi.mock('../../api/scheduled', () => ({ scheduledApi: { scheduleSend: vi.fn() } }));
vi.mock('../../api/messages', () => ({
  moveMessage: vi.fn(),
  deleteMessage: vi.fn(),
  flagMessage: vi.fn(),
  saveDraft: vi.fn(),
}));

import { backgroundSync } from '../../utils/background-sync';
import { PendingSyncBanner } from './PendingSyncBanner';

describe('PendingSyncBanner', () => {
  beforeEach(async () => {
    await backgroundSync.clearAll();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders nothing when queue is empty', async () => {
    const { container } = render(<PendingSyncBanner />);
    // Wait for the initial async getPendingCount to resolve
    await waitFor(() => {
      expect(container.querySelector('[data-testid="pending-sync-banner"]')).toBeNull();
    });
  });

  it('renders count and offline label when queue is non-empty and navigator is offline', async () => {
    // Force navigator.onLine = false BEFORE rendering so the hook seeds the
    // offline state on mount.
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });

    await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: 'Hi' });
    await backgroundSync.enqueue('move', { folder: 'INBOX', uid: 1, toFolder: 'Trash' });

    render(<PendingSyncBanner />);

    const banner = await screen.findByTestId('pending-sync-banner');
    expect(banner).toHaveTextContent('Pending sync:');
    expect(banner).toHaveTextContent('Offline — 2 actions queued');
    // Retry button is hidden when offline
    expect(screen.queryByTestId('pending-sync-retry')).toBeNull();

    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
  });

  it('shows singular "action" when count is 1', async () => {
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
    await backgroundSync.enqueue('flag', { folder: 'INBOX', uid: 1, flag: '\\Seen', add: true });

    render(<PendingSyncBanner />);
    const banner = await screen.findByTestId('pending-sync-banner');
    expect(banner).toHaveTextContent('Offline — 1 action queued');

    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
  });

  it('shows "Syncing" + Retry button when online with pending actions', async () => {
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
    await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });

    render(<PendingSyncBanner />);

    const banner = await screen.findByTestId('pending-sync-banner');
    expect(banner).toHaveTextContent('Syncing 1 action');
    expect(screen.getByTestId('pending-sync-retry')).toBeInTheDocument();
  });

  it('invokes processPending when Retry button is clicked', async () => {
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
    await backgroundSync.enqueue('delete', { folder: 'INBOX', uid: 1 });
    const spy = vi.spyOn(backgroundSync, 'processPending');

    render(<PendingSyncBanner />);
    await screen.findByTestId('pending-sync-banner');

    await act(async () => {
      fireEvent.click(screen.getByTestId('pending-sync-retry'));
    });

    await waitFor(() => expect(spy).toHaveBeenCalled());
  });

  it('updates count reactively when queue changes via subscribe', async () => {
    Object.defineProperty(navigator, 'onLine', { configurable: true, value: false });
    await backgroundSync.enqueue('send', { to: ['a@b.c'], subject: '1' });

    render(<PendingSyncBanner />);
    let banner = await screen.findByTestId('pending-sync-banner');
    expect(banner).toHaveTextContent('1 action');

    await act(async () => {
      await backgroundSync.enqueue('send', { to: ['d@e.f'], subject: '2' });
    });

    banner = await screen.findByTestId('pending-sync-banner');
    await waitFor(() => expect(banner).toHaveTextContent('2 actions'));

    Object.defineProperty(navigator, 'onLine', { configurable: true, value: true });
  });
});
