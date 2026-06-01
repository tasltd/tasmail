// TMAIL-404: regression coverage for the wizard's "Finish setup" handler.
// After SMTP creation succeeds, the wizard MUST:
//   1) Invalidate any stale ['folders'] / ['messages'] / ['quota'] cache so
//      a 503-cached error from a previous visit doesn't survive into /app.
//   2) Prefetch ['folders'] so AppShell's FolderTree finds the request
//      already in flight (or complete) when it mounts — otherwise the user
//      stares at "Loading folders…" for the full IMAP login round-trip.
//   3) Navigate to /app after a short success-screen delay.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockCreateImap = vi.fn();
const mockCreateSmtp = vi.fn();
const mockTestImap = vi.fn();
const mockPresets = vi.fn();
const mockListPublic = vi.fn();
const mockFetchFolders = vi.fn();
const mockNavigate = vi.fn();

vi.mock('../../api/byok', () => ({
  byokApi: {
    presets: (...args: unknown[]) => mockPresets(...args),
    createImap: (...args: unknown[]) => mockCreateImap(...args),
    createSmtp: (...args: unknown[]) => mockCreateSmtp(...args),
    testImap: (...args: unknown[]) => mockTestImap(...args),
  },
}));

vi.mock('../../api/featureFlags', () => ({
  featureFlagsApi: {
    listPublic: (...args: unknown[]) => mockListPublic(...args),
  },
}));

vi.mock('../../api/folders', () => ({
  fetchFolders: (...args: unknown[]) => mockFetchFolders(...args),
}));

vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return { ...actual, useNavigate: () => mockNavigate };
});

import { OnboardingWizard } from './OnboardingWizard';

function wrap(qc: QueryClient) {
  return (
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <OnboardingWizard />
      </MemoryRouter>
    </QueryClientProvider>
  );
}

function buildQueryClient() {
  return new QueryClient({ defaultOptions: { queries: { retry: false } } });
}

// Drive the wizard from the provider step (custom IMAP) all the way to the
// SMTP "Finish setup" click. The path picker auto-skips because we only set
// byok_onboarding_enabled in the feature flag stub.
async function walkToSmtpFinish() {
  await waitFor(() =>
    expect(screen.getByText('Who hosts your email?')).toBeInTheDocument(),
  );
  fireEvent.click(screen.getByText('Other / Custom'));

  await waitFor(() =>
    expect(screen.getByText(/IMAP server/i)).toBeInTheDocument(),
  );
  fireEvent.change(screen.getByPlaceholderText(/imap\./i), {
    target: { value: 'imap.example.com' },
  });
  fireEvent.change(screen.getByPlaceholderText(/full email/i), {
    target: { value: 'user@example.com' },
  });
  // Label/input aren't wired with htmlFor — fall back to the visible type attr.
  const imapPwd = document.querySelector('input[type="password"]') as HTMLInputElement;
  fireEvent.change(imapPwd, { target: { value: 'secret-pass' } });
  fireEvent.click(screen.getByRole('button', { name: /Save & continue/i }));

  await waitFor(() =>
    expect(screen.getByText(/SMTP server/i)).toBeInTheDocument(),
  );
  fireEvent.change(screen.getByPlaceholderText(/smtp\./i), {
    target: { value: 'smtp.example.com' },
  });
  fireEvent.change(screen.getByPlaceholderText(/full email/i), {
    target: { value: 'user@example.com' },
  });
  // Label/input aren't wired with htmlFor — fall back to the visible type attr.
  const smtpPwd = document.querySelector('input[type="password"]') as HTMLInputElement;
  fireEvent.change(smtpPwd, { target: { value: 'secret-pass' } });
  fireEvent.click(screen.getByRole('button', { name: /Finish setup/i }));
}

describe('OnboardingWizard finish handler (TMAIL-404)', () => {
  beforeEach(() => {
    vi.clearAllMocks();

    mockPresets.mockResolvedValue([]);
    mockListPublic.mockResolvedValue([
      { key: 'byok_onboarding_enabled', enabled: true },
      { key: 'dns_mx_onboarding_enabled', enabled: false },
    ]);
    mockCreateImap.mockResolvedValue({
      id: 'i1', name: 'My IMAP', host: 'imap.example.com', port: 993,
      username: 'user@example.com', encryption: 'ssl', is_default: true,
      verified: false, last_tested_at: null, last_error: null,
    });
    mockCreateSmtp.mockResolvedValue({
      id: 's1', name: 'My SMTP', host: 'smtp.example.com', port: 587,
      username: 'user@example.com', encryption: 'starttls', is_default: true,
      from_address: 'user@example.com', verified: false,
    });
    mockFetchFolders.mockResolvedValue([
      { name: 'INBOX', delimiter: '/', messages: 0, unseen: 0 },
    ]);
  });

  it('prefetches /api/folders so AppShell renders folders without a cold loading state', async () => {
    const qc = buildQueryClient();
    render(wrap(qc));
    await walkToSmtpFinish();

    await waitFor(() => expect(mockCreateSmtp).toHaveBeenCalledTimes(1));

    // The prefetch is the load-bearing fix: by warming the ['folders'] query
    // BEFORE navigate, AppShell's FolderTree attaches to an in-flight (or
    // already-resolved) request instead of triggering a cold fetch.
    await waitFor(() => expect(mockFetchFolders).toHaveBeenCalled());

    // And the prefetched data must actually land in the shared cache so the
    // FolderTree's useFolders reads it directly on mount.
    await waitFor(() =>
      expect(qc.getQueryData(['folders'])).toEqual([
        { name: 'INBOX', delimiter: '/', messages: 0, unseen: 0 },
      ]),
    );
  });

  it('navigates to /app after the success-screen delay', async () => {
    const qc = buildQueryClient();
    render(wrap(qc));
    await walkToSmtpFinish();
    await waitFor(() => expect(mockCreateSmtp).toHaveBeenCalledTimes(1));

    // The "done" overlay should render before the navigate fires.
    await waitFor(() =>
      expect(screen.getByText(/You're all set/i)).toBeInTheDocument(),
    );

    // navigate is fired from a 600 ms setTimeout — give it a real beat to land.
    await waitFor(
      () => expect(mockNavigate).toHaveBeenCalledWith('/app', { replace: true }),
      { timeout: 2_000 },
    );
  });

  it('drops a stale ["folders"] cache entry before navigating', async () => {
    const qc = buildQueryClient();
    // Simulate a stale folder list left behind from an earlier failed visit
    // (e.g. user landed on /app before completing onboarding, useFolders fired
    // and got 503, the error stayed in cache).
    qc.setQueryData(['folders'], [{ name: 'STALE', delimiter: '/', messages: 0, unseen: 0 }]);

    render(wrap(qc));
    await walkToSmtpFinish();
    await waitFor(() => expect(mockCreateSmtp).toHaveBeenCalledTimes(1));

    // After the wizard finishes, the cache must reflect the fresh prefetch,
    // not the stale "STALE" entry.
    await waitFor(() => {
      const data = qc.getQueryData(['folders']) as Array<{ name: string }>;
      expect(data).toEqual([
        { name: 'INBOX', delimiter: '/', messages: 0, unseen: 0 },
      ]);
    });
    expect(mockFetchFolders).toHaveBeenCalled();
  });
});
