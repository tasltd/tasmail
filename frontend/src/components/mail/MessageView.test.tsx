import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MessageView } from './MessageView';
import type { FullMessage } from '../../types/mail';

const mockUseCurrentMessage = vi.fn();
vi.mock('../../hooks/useMailbox', () => ({
  useCurrentMessage: () => mockUseCurrentMessage(),
}));

const mockSetSelectedUid = vi.fn();
const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      selectedFolder: 'INBOX',
      setSelectedUid: mockSetSelectedUid,
      setViewMode: mockSetViewMode,
    }),
}));

vi.mock('../../api/messages', () => ({
  deleteMessage: vi.fn(),
  moveMessage: vi.fn(),
  flagMessage: vi.fn(),
}));

vi.mock('../../utils/sanitize', () => ({
  sanitizeHtml: (html: string) => html,
}));

vi.mock('../../utils/date', () => ({
  formatFullDate: (d: string | null) => d ?? '',
}));

function makeMessage(overrides: Partial<FullMessage> = {}): FullMessage {
  return {
    uid: 1,
    subject: 'Test Subject',
    from: 'alice@example.com',
    to: ['bob@example.com'],
    cc: [],
    date: '2026-04-10T10:00:00Z',
    flags: [],
    text_body: 'Hello world',
    html_body: null,
    attachments: [],
    message_id: '<msg1@example.com>',
    in_reply_to: null,
    references: [],
    ...overrides,
  };
}

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('MessageView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows loading skeleton when loading', () => {
    mockUseCurrentMessage.mockReturnValue({ data: undefined, isLoading: true, error: null });
    render(<MessageView />, { wrapper });
    expect(document.querySelector('.loading-skeleton')).not.toBeNull();
  });

  it('shows error state', () => {
    mockUseCurrentMessage.mockReturnValue({ data: undefined, isLoading: false, error: new Error('fail') });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('Failed to load message')).toBeInTheDocument();
  });

  it('returns null when no message', () => {
    mockUseCurrentMessage.mockReturnValue({ data: null, isLoading: false, error: null });
    const { container } = render(<MessageView />, { wrapper });
    expect(container.innerHTML).toBe('');
  });

  it('renders message subject, from, to', () => {
    mockUseCurrentMessage.mockReturnValue({ data: makeMessage(), isLoading: false, error: null });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('Test Subject')).toBeInTheDocument();
    expect(screen.getByText('alice@example.com')).toBeInTheDocument();
    expect(screen.getByText('bob@example.com')).toBeInTheDocument();
  });

  it('shows "(no subject)" for empty subject', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ subject: null }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('(no subject)')).toBeInTheDocument();
  });

  it('renders cc when present', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ cc: ['charlie@example.com', 'dave@example.com'] }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('charlie@example.com, dave@example.com')).toBeInTheDocument();
  });

  it('does not render cc section when empty', () => {
    mockUseCurrentMessage.mockReturnValue({ data: makeMessage({ cc: [] }), isLoading: false, error: null });
    render(<MessageView />, { wrapper });
    expect(screen.queryByText('Cc:')).not.toBeInTheDocument();
  });

  it('renders text body when no html', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ text_body: 'Plain text content', html_body: null }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('Plain text content')).toBeInTheDocument();
  });

  it('renders html body when present', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ html_body: '<p>Rich HTML</p>' }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(document.querySelector('.message-view__html')).not.toBeNull();
  });

  it('renders attachments when present', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({
        attachments: [
          { filename: 'report.pdf', content_type: 'application/pdf', size: 102400, part_id: '2' },
          { filename: 'photo.jpg', content_type: 'image/jpeg', size: 51200, part_id: '3' },
        ],
      }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByText('Attachments:')).toBeInTheDocument();
    expect(screen.getByText('report.pdf (100KB)')).toBeInTheDocument();
    expect(screen.getByText('photo.jpg (50KB)')).toBeInTheDocument();
  });

  it('does not render attachments section when empty', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ attachments: [] }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.queryByText('Attachments:')).not.toBeInTheDocument();
  });

  it('clicking Reply sets viewMode to compose', () => {
    mockUseCurrentMessage.mockReturnValue({ data: makeMessage(), isLoading: false, error: null });
    render(<MessageView />, { wrapper });
    fireEvent.click(screen.getByTitle('Reply'));
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
  });

  it('clicking Back clears selected uid', () => {
    mockUseCurrentMessage.mockReturnValue({ data: makeMessage(), isLoading: false, error: null });
    render(<MessageView />, { wrapper });
    fireEvent.click(screen.getByTitle('Back to list'));
    expect(mockSetSelectedUid).toHaveBeenCalledWith(null);
  });

  it('shows Star button title based on flagged state', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ flags: ['\\Flagged'] }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByTitle('Unstar')).toBeInTheDocument();
  });

  it('shows Star title when not flagged', () => {
    mockUseCurrentMessage.mockReturnValue({
      data: makeMessage({ flags: [] }),
      isLoading: false,
      error: null,
    });
    render(<MessageView />, { wrapper });
    expect(screen.getByTitle('Star')).toBeInTheDocument();
  });

  it('has Delete and Forward buttons', () => {
    mockUseCurrentMessage.mockReturnValue({ data: makeMessage(), isLoading: false, error: null });
    render(<MessageView />, { wrapper });
    expect(screen.getByTitle('Delete')).toBeInTheDocument();
    expect(screen.getByTitle('Forward')).toBeInTheDocument();
    expect(screen.getByTitle('Move to folder')).toBeInTheDocument();
  });
});
