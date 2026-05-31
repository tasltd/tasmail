import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MessageList } from './MessageList';

// Mock the hooks
const mockUseCurrentMessages = vi.fn();
vi.mock('../../hooks/useMailbox', () => ({
  useCurrentMessages: () => mockUseCurrentMessages(),
}));

const mockSelectedUid = vi.fn<() => number | null>(() => null);
const mockSetSelectedUid = vi.fn();
// TMAIL-401: store mock now includes selectedFolder so MessageList's
// "render EmptyInboxState for INBOX" branch can be exercised.
const mockSelectedFolder = vi.fn<() => string>(() => 'Sent');
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      selectedUid: mockSelectedUid(),
      setSelectedUid: mockSetSelectedUid,
      selectedFolder: mockSelectedFolder(),
    }),
}));

// TMAIL-401: stub EmptyInboxState so we don't need to mock the byok query
// inside MessageList tests — its own test file covers the rendering.
vi.mock('./EmptyInboxState', () => ({
  EmptyInboxState: () => <div data-testid="empty-inbox-state-stub">Empty inbox</div>,
}));

// Mock formatMessageDate to return the date string as-is for predictable assertions
vi.mock('../../utils/date', () => ({
  formatMessageDate: (d: string | null) => d || '',
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

// Added: Helper to build a message envelope for tests
function makeMessage(overrides: Partial<{
  uid: number;
  from: string | null;
  subject: string | null;
  date: string | null;
  flags: string[];
}> = {}) {
  return {
    uid: overrides.uid ?? 1,
    from: overrides.from !== undefined ? overrides.from : 'alice@example.com',
    subject: overrides.subject !== undefined ? overrides.subject : 'Hello',
    date: overrides.date !== undefined ? overrides.date : '2026-04-10',
    flags: overrides.flags ?? [],
  };
}

describe('MessageList', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockSelectedUid.mockReturnValue(null);
    mockSelectedFolder.mockReturnValue('Sent');
  });

  it('shows loading skeleton when loading', () => {
    mockUseCurrentMessages.mockReturnValue({ data: undefined, isLoading: true, error: null });
    render(<MessageList />, { wrapper });
    // LoadingSkeleton renders placeholder rows
    expect(screen.queryByText('Failed to load messages')).not.toBeInTheDocument();
    expect(screen.queryByText('No messages in this folder')).not.toBeInTheDocument();
  });

  it('shows error message on error', () => {
    mockUseCurrentMessages.mockReturnValue({ data: undefined, isLoading: false, error: new Error('fail') });
    render(<MessageList />, { wrapper });
    expect(screen.getByText('Failed to load messages')).toBeInTheDocument();
  });

  it('shows empty state when no messages', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: { messages: [], total: 0 },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByText('No messages in this folder')).toBeInTheDocument();
  });

  // TMAIL-401: INBOX gets the richer EmptyInboxState component instead of
  // the bare "No messages" string.
  it('shows EmptyInboxState when INBOX is selected and there are no messages', () => {
    mockSelectedFolder.mockReturnValue('INBOX');
    mockUseCurrentMessages.mockReturnValue({
      data: { messages: [], total: 0 },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByTestId('empty-inbox-state-stub')).toBeInTheDocument();
    expect(screen.queryByText('No messages in this folder')).not.toBeInTheDocument();
  });

  // TMAIL-402: A brand-new BYOK user whose IMAP isn't yet reachable should
  // see the welcoming EmptyInboxState on INBOX (with their configured
  // user@host), not the raw "Failed to load messages" string. Non-INBOX
  // folders keep the error message — see the next test.
  it('shows EmptyInboxState on INBOX even when the messages query errors', () => {
    mockSelectedFolder.mockReturnValue('INBOX');
    mockUseCurrentMessages.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('IMAP unreachable'),
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByTestId('empty-inbox-state-stub')).toBeInTheDocument();
    expect(screen.queryByText('Failed to load messages')).not.toBeInTheDocument();
  });

  // TMAIL-402: Non-INBOX folders still surface the error message — only the
  // INBOX gets the welcoming-empty-state fallback.
  it('still shows "Failed to load messages" for non-INBOX folders on error', () => {
    mockSelectedFolder.mockReturnValue('Sent');
    mockUseCurrentMessages.mockReturnValue({
      data: undefined,
      isLoading: false,
      error: new Error('IMAP unreachable'),
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByText('Failed to load messages')).toBeInTheDocument();
    expect(screen.queryByTestId('empty-inbox-state-stub')).not.toBeInTheDocument();
  });

  it('renders message rows with from, subject, date', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, from: 'alice@example.com', subject: 'Hello', date: '2026-04-10', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, from: 'bob@example.com', subject: 'Meeting', date: '2026-04-09', flags: ['\\Seen'] }),
        ],
        total: 2,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('Hello')).toBeInTheDocument();
    expect(screen.getByText('2026-04-10')).toBeInTheDocument();
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
    expect(screen.getByText('Meeting')).toBeInTheDocument();
  });

  it('marks unread messages with unread class', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, flags: [] }),
          makeMessage({ uid: 2, flags: ['\\Seen'] }),
        ],
        total: 2,
      },
      isLoading: false,
      error: null,
    });
    // NOTE: Threading is on by default; these have same subject so they group.
    // Disable threading by unchecking the checkbox first.
    render(<MessageList />, { wrapper });
    const checkbox = screen.getByLabelText('Conversations');
    fireEvent.click(checkbox);

    // Changed: Filter to only message-row buttons (excludes toolbar buttons like EML import)
    const rows = screen.getAllByRole('button').filter((el) => el.className.includes('message-row'));
    // First row (uid=1) should be unread
    expect(rows[0].className).toContain('message-row--unread');
    // Second row (uid=2) should be read
    expect(rows[1].className).not.toContain('message-row--unread');
  });

  it('marks active message based on selectedUid', () => {
    mockSelectedUid.mockReturnValue(2);
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: 'First', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, subject: 'Second', flags: ['\\Seen'] }),
        ],
        total: 2,
      },
      isLoading: false,
      error: null,
    });
    // Disable threading so each message renders as its own MessageRow
    render(<MessageList />, { wrapper });
    const checkbox = screen.getByLabelText('Conversations');
    fireEvent.click(checkbox);

    const activeRow = screen.getByText('Second').closest('[role="button"]');
    expect(activeRow?.className).toContain('message-row--active');

    const inactiveRow = screen.getByText('First').closest('[role="button"]');
    expect(inactiveRow?.className).not.toContain('message-row--active');
  });

  it('calls setSelectedUid when message row clicked', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 42, subject: 'Clickable', flags: ['\\Seen'] }),
        ],
        total: 1,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    fireEvent.click(screen.getByText('Clickable'));
    expect(mockSetSelectedUid).toHaveBeenCalledWith(42);
  });

  it('groups messages by subject when threading enabled', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: 'Project Update', from: 'alice@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, subject: 'Re: Project Update', from: 'bob@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 3, subject: 'Other Topic', from: 'carol@example.com', flags: ['\\Seen'] }),
        ],
        total: 3,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    // Threading is on by default — "Project Update" thread groups 2 messages
    // "Other Topic" renders as a single MessageRow
    expect(screen.getByText('carol@example.com')).toBeInTheDocument();
    expect(screen.getByText('Other Topic')).toBeInTheDocument();
  });

  it('shows thread count in parentheses', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: 'Thread Subject', from: 'alice@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, subject: 'Re: Thread Subject', from: 'bob@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 3, subject: 'Fwd: Thread Subject', from: 'carol@example.com', flags: ['\\Seen'] }),
        ],
        total: 3,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    // ThreadRow shows "(3)" for three grouped messages
    expect(screen.getByText('(3)')).toBeInTheDocument();
  });

  it('expands thread to show individual messages on click', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: 'Discussion', from: 'alice@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, subject: 'Re: Discussion', from: 'bob@example.com', flags: ['\\Seen'] }),
        ],
        total: 2,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });

    // Before expanding, only the thread header is visible (first from)
    expect(screen.getByText('(2)')).toBeInTheDocument();

    // Click the thread header to expand
    const threadHeader = screen.getByText('(2)').closest('[role="button"]')!;
    fireEvent.click(threadHeader);

    // After expanding, both individual messages should be visible
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
    // Click an expanded message to select it
    fireEvent.click(screen.getByText('bob@example.com'));
    expect(mockSetSelectedUid).toHaveBeenCalledWith(2);
  });

  it('toggles threading off via checkbox', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: 'Topic', from: 'alice@example.com', flags: ['\\Seen'] }),
          makeMessage({ uid: 2, subject: 'Re: Topic', from: 'bob@example.com', flags: ['\\Seen'] }),
        ],
        total: 2,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });

    // Threading on by default — grouped into one thread row
    expect(screen.getByText('(2)')).toBeInTheDocument();

    // Uncheck Conversations
    const checkbox = screen.getByLabelText('Conversations');
    expect(checkbox).toBeChecked();
    fireEvent.click(checkbox);

    // Now individual message rows are shown, thread count disappears
    expect(screen.queryByText('(2)')).not.toBeInTheDocument();
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
  });

  it('handles null subject and from gracefully', () => {
    mockUseCurrentMessages.mockReturnValue({
      data: {
        messages: [
          makeMessage({ uid: 1, subject: null, from: null, flags: ['\\Seen'] }),
        ],
        total: 1,
      },
      isLoading: false,
      error: null,
    });
    render(<MessageList />, { wrapper });
    expect(screen.getByText('(unknown)')).toBeInTheDocument();
    expect(screen.getByText('(no subject)')).toBeInTheDocument();
  });
});
