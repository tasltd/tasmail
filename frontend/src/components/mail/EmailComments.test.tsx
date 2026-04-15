// Added: EmailComments component tests for TMAIL-128

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EmailComments } from './EmailComments';

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

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('EmailComments', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Comments heading', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Comments')).toBeInTheDocument();
    });
  });

  it('shows empty state when no comments exist', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No comments yet. Be the first to add one.')).toBeInTheDocument();
    });
  });

  it('renders comment list with author and content', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'Please review this email',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
      {
        id: 'c2',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'Looks good to me',
        author_name: 'Bob',
        author_email: 'bob@example.com',
        created_at: '2026-04-14T11:00:00Z',
        updated_at: '2026-04-14T11:00:00Z',
      },
    ]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Alice')).toBeInTheDocument();
      expect(screen.getByText('Please review this email')).toBeInTheDocument();
      expect(screen.getByText('Bob')).toBeInTheDocument();
      expect(screen.getByText('Looks good to me')).toBeInTheDocument();
    });
  });

  it('shows comment count badge when comments exist', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'First comment',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('1')).toBeInTheDocument();
    });
  });

  it('renders add comment input and send button', async () => {
    mockFetchComments.mockResolvedValue([]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByPlaceholderText('Add a comment...')).toBeInTheDocument();
      expect(screen.getByTitle('Send')).toBeInTheDocument();
    });
  });

  it('calls createComment when form is submitted', async () => {
    mockFetchComments.mockResolvedValue([]);
    mockCreateComment.mockResolvedValue({
      id: 'c-new',
      mailbox_id: 'mb1',
      message_uid: 1,
      folder: 'INBOX',
      content: 'New comment text',
      author_name: 'User',
      author_email: 'user@example.com',
      created_at: '2026-04-14T12:00:00Z',
      updated_at: '2026-04-14T12:00:00Z',
    });
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByPlaceholderText('Add a comment...')).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText('Add a comment...'), {
      target: { value: 'New comment text' },
    });
    fireEvent.click(screen.getByTitle('Send'));

    await waitFor(() => {
      expect(mockCreateComment).toHaveBeenCalledWith('INBOX', 1, { content: 'New comment text' });
    });
  });

  it('renders edit and delete buttons for each comment', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'A comment',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Edit')).toBeInTheDocument();
      expect(screen.getByTitle('Delete')).toBeInTheDocument();
    });
  });

  it('enters edit mode when edit button is clicked', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'Editable comment',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Edit')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Edit'));

    // NOTE: Edit mode shows Save and Cancel buttons
    expect(screen.getByTitle('Save')).toBeInTheDocument();
    expect(screen.getByTitle('Cancel')).toBeInTheDocument();
  });

  it('calls deleteComment when delete button is clicked', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'Delete me',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    mockDeleteComment.mockResolvedValue(undefined);
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Delete')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Delete'));

    await waitFor(() => {
      expect(mockDeleteComment).toHaveBeenCalled();
      expect(mockDeleteComment.mock.calls[0][0]).toBe('c1');
    });
  });

  it('calls updateComment when edit form is submitted', async () => {
    mockFetchComments.mockResolvedValue([
      {
        id: 'c1',
        mailbox_id: 'mb1',
        message_uid: 1,
        folder: 'INBOX',
        content: 'Original text',
        author_name: 'Alice',
        author_email: 'alice@example.com',
        created_at: '2026-04-14T10:00:00Z',
        updated_at: '2026-04-14T10:00:00Z',
      },
    ]);
    mockUpdateComment.mockResolvedValue({
      id: 'c1',
      content: 'Updated text',
      author_name: 'Alice',
      author_email: 'alice@example.com',
      created_at: '2026-04-14T10:00:00Z',
      updated_at: '2026-04-14T12:00:00Z',
    });
    render(<EmailComments folder="INBOX" uid={1} />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Edit')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Edit'));

    // NOTE: The input should be pre-filled with the original content
    const editInput = screen.getByDisplayValue('Original text');
    fireEvent.change(editInput, { target: { value: 'Updated text' } });
    fireEvent.click(screen.getByTitle('Save'));

    await waitFor(() => {
      expect(mockUpdateComment).toHaveBeenCalledWith('c1', { content: 'Updated text' });
    });
  });
});
