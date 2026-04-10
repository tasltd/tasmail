import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Composer } from './Composer';

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
  beforeEach(() => {
    vi.clearAllMocks();
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
      vi.advanceTimersByTime(5000);
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
      vi.advanceTimersByTime(3000);
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
