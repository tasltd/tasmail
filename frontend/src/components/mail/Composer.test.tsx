import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
// Added (TMAIL-89): the Composer now writes drafts to IndexedDB via the
// offline-drafts module. Jsdom doesn't ship `indexedDB`, so polyfill it.
import 'fake-indexeddb/auto';
import { Composer } from './Composer';
import { offlineCache } from '../../utils/offline-cache';
import { clearSessionKey } from '../../utils/offline-encryption';

// Mock TipTap editor
vi.mock('@tiptap/react', () => ({
  useEditor: () => ({
    getHTML: () => '<p>Test email body</p>',
    getText: () => 'Test email body',
    on: vi.fn(),
    off: vi.fn(),
  }),
  EditorContent: ({ editor }: { editor: unknown }) =>
    editor ? <div data-testid="editor-content">Editor</div> : null,
}));

vi.mock('@tiptap/starter-kit', () => ({ default: {} }));
vi.mock('@tiptap/extension-link', () => ({ default: { configure: () => ({}) } }));
vi.mock('@tiptap/extension-placeholder', () => ({ default: { configure: () => ({}) } }));

const mockSaveDraft = vi.fn();
vi.mock('../../api/messages', () => ({
  saveDraft: (...args: unknown[]) => mockSaveDraft(...args),
}));

const mockScheduleSend = vi.fn();
const mockCancelScheduled = vi.fn();
vi.mock('../../api/scheduled', () => ({
  scheduledApi: {
    scheduleSend: (...args: unknown[]) => mockScheduleSend(...args),
    cancelScheduled: (...args: unknown[]) => mockCancelScheduled(...args),
  },
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

// Added (TMAIL-89): partial mock of offline-drafts so the auto-save tests
// don't deadlock fake-indexeddb against vitest fake timers. We keep all the
// pure helpers (statusBadge, applyEdits, createEmptyDraft, …) intact and only
// replace the IndexedDB-backed persistence with in-memory stubs.
vi.mock('../../utils/offline-drafts', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../utils/offline-drafts')>();
  const memDrafts = new Map<string, import('../../utils/offline-drafts').OfflineDraft>();
  return {
    ...actual,
    saveDraftLocal: async (draft: import('../../utils/offline-drafts').OfflineDraft) => {
      memDrafts.set(draft.localId, draft);
    },
    loadDraft: async (localId: string) => memDrafts.get(localId) ?? null,
    deleteDraftLocal: async (localId: string) => {
      memDrafts.delete(localId);
    },
    listLocalDrafts: async () => Array.from(memDrafts.values()),
    addAttachment: async (draft: import('../../utils/offline-drafts').OfflineDraft, file: { name: string; type: string; size: number; blob: Blob }) => {
      const attachment = {
        id: `att-${memDrafts.size}-${file.name}`,
        filename: file.name,
        mimeType: file.type,
        size: file.size,
        addedAt: Date.now(),
      };
      const next = { ...draft, attachments: [...draft.attachments, attachment], lastEditedAt: Date.now() };
      return { attachment, draft: next };
    },
    removeAttachment: async (draft: import('../../utils/offline-drafts').OfflineDraft, attachmentId: string) =>
      ({ ...draft, attachments: draft.attachments.filter((a) => a.id !== attachmentId), lastEditedAt: Date.now() }),
    loadAttachmentBlob: async () => null,
    syncOne: async (
      draft: import('../../utils/offline-drafts').OfflineDraft,
      ctx: import('../../utils/offline-drafts').SyncContext,
    ) => {
      try {
        const result = await ctx.postDraft(draft);
        if (result.status === 'ok') {
          return { ...draft, status: 'synced' as const, syncedVersion: draft.lastEditedAt, lastSyncedAt: Date.now() };
        }
        return { ...draft, status: 'conflict' as const, serverConflictVersion: result.serverVersion };
      } catch (e) {
        return { ...draft, status: 'error' as const, errorMessage: e instanceof Error ? e.message : 'err' };
      }
    },
    _resetAttachmentsForTests: async () => {
      memDrafts.clear();
    },
  };
});

// NOTE: must come after the vi.mock above so the test imports the stubbed
// version, not the real IDB one.
const { _resetAttachmentsForTests } = await import('../../utils/offline-drafts');

// Added: Mock calendar API for the Schedule Meeting modal flow (TMAIL-127)
const mockCreateEvent = vi.fn();
vi.mock('../../api/calendar', () => ({
  createEvent: (...args: unknown[]) => mockCreateEvent(...args),
}));

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('Composer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders composer with To, Cc, Subject fields', () => {
    render(<Composer />, { wrapper });

    expect(screen.getByPlaceholderText('recipient@example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('cc@example.com')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Subject')).toBeInTheDocument();
  });

  it('renders Send and Schedule buttons', () => {
    render(<Composer />, { wrapper });

    expect(screen.getByText('Send')).toBeInTheDocument();
    expect(screen.getByText('Schedule')).toBeInTheDocument();
  });

  it('renders New Message header', () => {
    render(<Composer />, { wrapper });
    expect(screen.getByText('New Message')).toBeInTheDocument();
  });

  it('renders TipTap editor', () => {
    render(<Composer />, { wrapper });
    expect(screen.getByTestId('editor-content')).toBeInTheDocument();
  });

  it('shows error when sending without recipients', async () => {
    render(<Composer />, { wrapper });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(screen.getByText('Recipients required')).toBeInTheDocument();
    expect(mockScheduleSend).not.toHaveBeenCalled();
  });

  it('sends email with undo-send (10s delay)', async () => {
    mockScheduleSend.mockResolvedValue({ cancel_token: 'tok-123' });

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'test@example.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('Subject'), {
      target: { value: 'Test Subject' },
    });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(mockScheduleSend).toHaveBeenCalledWith(
      expect.objectContaining({
        to: ['test@example.com'],
        subject: 'Test Subject',
        delay_seconds: 10,
      }),
    );
  });

  it('shows undo toast after sending', async () => {
    mockScheduleSend.mockResolvedValue({ cancel_token: 'tok-123' });

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'user@test.com' },
    });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(screen.getByText('Undo')).toBeInTheDocument();
    expect(screen.getByText(/Message sent/)).toBeInTheDocument();
  });

  it('handles send failure gracefully', async () => {
    mockScheduleSend.mockRejectedValue(new Error('SMTP error'));

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'user@test.com' },
    });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(screen.getByText('SMTP error')).toBeInTheDocument();
  });

  it('toggles schedule picker on Schedule button click', () => {
    render(<Composer />, { wrapper });

    expect(screen.queryByText('Send at:')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('Schedule'));

    expect(screen.getByText('Send at:')).toBeInTheDocument();
    expect(screen.getByText('Schedule Send')).toBeInTheDocument();
  });

  it('closes composer when X button is clicked', () => {
    render(<Composer />, { wrapper });

    const buttons = screen.getAllByRole('button');
    // X button is the second icon button in toolbar (after Save draft)
    fireEvent.click(buttons[1]);

    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('splits multiple recipients by comma', async () => {
    mockScheduleSend.mockResolvedValue({ cancel_token: 'tok' });

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'a@test.com, b@test.com, c@test.com' },
    });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(mockScheduleSend).toHaveBeenCalledWith(
      expect.objectContaining({
        to: ['a@test.com', 'b@test.com', 'c@test.com'],
      }),
    );
  });

  // Added: Schedule Meeting button + modal flow (TMAIL-127)
  it('opens Schedule Meeting modal pre-populated with To/Cc recipients and subject as title', async () => {
    mockCreateEvent.mockResolvedValue({
      event: {
        id: 'evt-1',
        title: 'Project sync',
        ics_uid: 'ics-1',
      },
      attendees: [],
    });

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'alice@example.com, bob@example.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('cc@example.com'), {
      target: { value: 'carol@example.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('Subject'), {
      target: { value: 'Project sync' },
    });

    fireEvent.click(screen.getByTestId('schedule-meeting-toggle'));

    // Modal should be present with subject as initial title
    const dialog = screen.getByRole('dialog', { name: /schedule meeting/i });
    expect(dialog).toBeInTheDocument();
    const titleInput = screen.getByLabelText('Title') as HTMLInputElement;
    expect(titleInput.value).toBe('Project sync');

    // Initial attendees from To + Cc should be listed
    const attendeesList = screen.getByLabelText('Attendees', { selector: 'ul' });
    expect(attendeesList).toHaveTextContent('alice@example.com');
    expect(attendeesList).toHaveTextContent('bob@example.com');
    expect(attendeesList).toHaveTextContent('carol@example.com');

    // Fill required start/end times
    fireEvent.change(screen.getByLabelText('Start'), {
      target: { value: '2026-06-10T10:00' },
    });
    fireEvent.change(screen.getByLabelText('End'), {
      target: { value: '2026-06-10T11:00' },
    });

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /create event/i }));
    });

    expect(mockCreateEvent).toHaveBeenCalledTimes(1);
    const payload = mockCreateEvent.mock.calls[0][0];
    expect(payload.title).toBe('Project sync');
    expect(payload.start_time).toBe(new Date('2026-06-10T10:00').toISOString());
    expect(payload.end_time).toBe(new Date('2026-06-10T11:00').toISOString());
    expect(payload.attendees).toEqual([
      { email: 'alice@example.com' },
      { email: 'bob@example.com' },
      { email: 'carol@example.com' },
    ]);

    // Modal should close on success
    expect(screen.queryByRole('dialog', { name: /schedule meeting/i })).not.toBeInTheDocument();
  });

  it('handles CC recipients', async () => {
    mockScheduleSend.mockResolvedValue({ cancel_token: 'tok' });

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'to@test.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('cc@example.com'), {
      target: { value: 'cc@test.com' },
    });

    await act(async () => {
      fireEvent.click(screen.getByText('Send'));
    });

    expect(mockScheduleSend).toHaveBeenCalledWith(
      expect.objectContaining({
        to: ['to@test.com'],
        cc: ['cc@test.com'],
      }),
    );
  });
});

describe('Composer auto-save drafts', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    // Reset IndexedDB between tests so the offline draft module starts from
    // a clean slate (TMAIL-89).
    await offlineCache.clearAll();
    await clearSessionKey();
    await _resetAttachmentsForTests();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('does not auto-save immediately on input', () => {
    mockSaveDraft.mockResolvedValue(undefined);

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'user@test.com' },
    });

    expect(mockSaveDraft).not.toHaveBeenCalled();
  });

  it('auto-saves draft after 5 second debounce', async () => {
    mockSaveDraft.mockResolvedValue(undefined);

    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'user@test.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('Subject'), {
      target: { value: 'Draft subject' },
    });

    await act(async () => {
      // TMAIL-89: advanceTimersByTimeAsync drains microtasks between fake
      // timer ticks so the refactored saveDraftNow's IDB writes resolve.
      // runAllTimersAsync would infinite-loop against useOnlineStatus's 30s
      // setInterval, so we explicitly advance only the 5s debounce window.
      await vi.advanceTimersByTimeAsync(5000);
    });

    expect(mockSaveDraft).toHaveBeenCalledWith(
      expect.objectContaining({
        to: ['user@test.com'],
        subject: 'Draft subject',
      }),
    );
  });

  it('does not auto-save when both to and subject are empty', async () => {
    mockSaveDraft.mockResolvedValue(undefined);

    render(<Composer />, { wrapper });

    // Only change CC which doesn't trigger save alone
    fireEvent.change(screen.getByPlaceholderText('cc@example.com'), {
      target: { value: 'cc@test.com' },
    });

    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    expect(mockSaveDraft).not.toHaveBeenCalled();
  });

  it('debounces multiple rapid changes', async () => {
    mockSaveDraft.mockResolvedValue(undefined);

    render(<Composer />, { wrapper });

    // Rapid changes should reset the timer each time
    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'a@test.com' },
    });

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'b@test.com' },
    });

    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // Should not have saved yet (only 2s since last change)
    expect(mockSaveDraft).not.toHaveBeenCalled();

    await act(async () => {
      // TMAIL-89: as above, drain microtasks for the async IDB pipeline.
      await vi.advanceTimersByTimeAsync(3000);
    });

    // Now 5s since last change, should save with latest value
    expect(mockSaveDraft).toHaveBeenCalledTimes(1);
    expect(mockSaveDraft).toHaveBeenCalledWith(
      expect.objectContaining({
        to: ['b@test.com'],
      }),
    );
  });
});

// Added (TMAIL-89): offline draft + status indicator + attachment behaviour.
describe('Composer offline drafts (TMAIL-89)', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    await _resetAttachmentsForTests();
  });

  it('renders the "Saved locally" status pill on first mount', async () => {
    render(<Composer />, { wrapper });
    const pill = await screen.findByTestId('draft-sync-status');
    expect(pill.textContent).toMatch(/saved locally/i);
  });

  it('flips the status pill to "Synced to server" after a successful sync', async () => {
    mockSaveDraft.mockResolvedValue(undefined);
    render(<Composer />, { wrapper });

    fireEvent.change(screen.getByPlaceholderText('recipient@example.com'), {
      target: { value: 'sync@example.com' },
    });
    fireEvent.change(screen.getByPlaceholderText('Subject'), {
      target: { value: 'Status pill check' },
    });

    // Click "Save draft now" to bypass the 5s debounce — same code path as
    // the debounced timer, just fires immediately.
    await act(async () => {
      fireEvent.click(screen.getByLabelText('Save draft now'));
    });

    await waitFor(() => expect(mockSaveDraft).toHaveBeenCalled());
    await waitFor(() => {
      const pill = screen.getByTestId('draft-sync-status');
      expect(pill.textContent).toMatch(/synced to server/i);
    });
  });

  it('exposes an attachment picker that queues files in the draft', async () => {
    render(<Composer />, { wrapper });

    const input = screen.getByTestId('composer-attachment-input') as HTMLInputElement;
    const file = new File(['hello bytes'], 'note.txt', { type: 'text/plain' });

    await act(async () => {
      // Simulate the user picking a file by setting the input's files via
      // Object.defineProperty (jsdom doesn't allow direct assignment).
      Object.defineProperty(input, 'files', { value: [file], writable: false });
      fireEvent.change(input);
    });

    const list = await screen.findByTestId('composer-attachment-list');
    expect(list).toHaveTextContent('note.txt');
  });
});
