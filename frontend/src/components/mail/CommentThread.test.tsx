// Added: Tests for CommentThread component (TMAIL-128)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CommentThread } from './CommentThread';

// Added: Mock the comments API
const mockFetchComments = vi.fn();
const mockCreateComment = vi.fn();
const mockUpdateComment = vi.fn();
const mockDeleteComment = vi.fn();

vi.mock('../../api/comments', () => ({
  fetchComments: (...args: unknown[]) => mockFetchComments(...args),
  createComment: (...args: unknown[]) => mockCreateComment(...args),
  updateComment: (...args: unknown[]) => mockUpdateComment(...args),
  deleteComment: (...args: unknown[]) => mockDeleteComment(...args),
}));

vi.mock('../../utils/date', () => ({
  formatFullDate: (d: string | null) => d ?? '',
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

const sampleComments = [
  {
    id: 'c1',
    mailbox_id: 'mb1',
    message_uid: 42,
    folder: 'INBOX',
    content: 'Need to follow up',
    author_name: 'Kwame Mensah',
    author_email: 'kwame@example.com',
    created_at: '2026-04-10T10:00:00Z',
    updated_at: '2026-04-10T10:00:00Z',
  },
  {
    id: 'c2',
    mailbox_id: 'mb1',
    message_uid: 42,
    folder: 'INBOX',
    content: 'Client confirmed receipt',
    author_name: 'Ama Adjei',
    author_email: 'ama@example.com',
    created_at: '2026-04-10T11:00:00Z',
    updated_at: '2026-04-10T11:00:00Z',
  },
];

describe('CommentThread', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockFetchComments.mockResolvedValue([]);
  });

  it('renders collapsed by default with toggle button', () => {
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });
    expect(screen.getByTestId('comment-toggle')).toBeInTheDocument();
    expect(screen.queryByTestId('comment-body')).not.toBeInTheDocument();
  });

  it('expands when toggle is clicked and shows empty state', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-body')).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getByTestId('comment-empty')).toBeInTheDocument();
    });
  });

  it('displays comments when expanded and data is loaded', async () => {
    mockFetchComments.mockResolvedValue(sampleComments);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getAllByTestId('comment-item')).toHaveLength(2);
    });
    expect(screen.getByText('Need to follow up')).toBeInTheDocument();
    expect(screen.getByText('Kwame Mensah')).toBeInTheDocument();
    expect(screen.getByText('Client confirmed receipt')).toBeInTheDocument();
    expect(screen.getByText('Ama Adjei')).toBeInTheDocument();
  });

  it('shows the new comment form when expanded', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-new-input')).toBeInTheDocument();
    });
    expect(screen.getByTestId('comment-submit-btn')).toBeInTheDocument();
  });

  it('submit button is disabled when input is empty', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-submit-btn')).toBeDisabled();
    });
  });

  it('submit button is enabled when input has content', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-new-input')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByTestId('comment-new-input'), {
      target: { value: 'A new comment' },
    });

    expect(screen.getByTestId('comment-submit-btn')).not.toBeDisabled();
  });

  it('shows edit and delete buttons on each comment', async () => {
    mockFetchComments.mockResolvedValue([sampleComments[0]]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-edit-btn')).toBeInTheDocument();
    });
    expect(screen.getByTestId('comment-delete-btn')).toBeInTheDocument();
  });

  it('enters edit mode when edit button is clicked', async () => {
    mockFetchComments.mockResolvedValue([sampleComments[0]]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('comment-toggle'));

    await waitFor(() => {
      expect(screen.getByTestId('comment-edit-btn')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('comment-edit-btn'));

    expect(screen.getByTestId('comment-edit-input')).toBeInTheDocument();
    expect(screen.getByTestId('comment-save-btn')).toBeInTheDocument();
    expect(screen.getByTestId('comment-cancel-btn')).toBeInTheDocument();
  });

  it('collapses when toggle is clicked a second time', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<CommentThread folder="INBOX" uid={42} />, { wrapper: createWrapper() });

    // Expand
    fireEvent.click(screen.getByTestId('comment-toggle'));
    await waitFor(() => {
      expect(screen.getByTestId('comment-body')).toBeInTheDocument();
    });

    // Collapse
    fireEvent.click(screen.getByTestId('comment-toggle'));
    expect(screen.queryByTestId('comment-body')).not.toBeInTheDocument();
  });
});
