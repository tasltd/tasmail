// Added: Tests for TaskManager component (TMAIL-126)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { TaskManager } from './TaskManager';

const mockListTasks = vi.fn();
const mockCreateTask = vi.fn();
const mockUpdateTask = vi.fn();
const mockDeleteTask = vi.fn();

vi.mock('../../api/tasks', () => ({
  listTasks: (...args: unknown[]) => mockListTasks(...args),
  createTask: (...args: unknown[]) => mockCreateTask(...args),
  updateTask: (...args: unknown[]) => mockUpdateTask(...args),
  deleteTask: (...args: unknown[]) => mockDeleteTask(...args),
}));

const mockSetViewMode = vi.fn();
vi.mock('../../stores/mailStore', () => ({
  useMailStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({ setViewMode: mockSetViewMode }),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('TaskManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and Add Task button', async () => {
    mockListTasks.mockResolvedValue([]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Tasks')).toBeInTheDocument();
    });
    expect(screen.getByText('Add Task')).toBeInTheDocument();
  });

  it('shows empty state message when no tasks', async () => {
    mockListTasks.mockResolvedValue([]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No tasks yet. Add one to get started.')).toBeInTheDocument();
    });
  });

  it('renders task list with titles and priorities', async () => {
    mockListTasks.mockResolvedValue([
      { id: '1', title: 'Reply to client', priority: 'high', completed: false, due_date: null, linked_subject: null },
      { id: '2', title: 'Send report', priority: 'normal', completed: true, due_date: null, linked_subject: null },
    ]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Reply to client')).toBeInTheDocument();
    });
    expect(screen.getByText('Send report')).toBeInTheDocument();
  });

  it('shows add task form when Add Task is clicked', async () => {
    mockListTasks.mockResolvedValue([]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Task')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Task'));
    expect(screen.getByText('New Task')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Task title')).toBeInTheDocument();
  });

  it('shows filter tabs: All, Active, Completed', async () => {
    mockListTasks.mockResolvedValue([]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('all')).toBeInTheDocument();
    });
    expect(screen.getByText('active')).toBeInTheDocument();
    expect(screen.getByText('completed')).toBeInTheDocument();
  });

  it('shows priority badges on tasks', async () => {
    mockListTasks.mockResolvedValue([
      { id: '1', title: 'Urgent task', priority: 'urgent', completed: false, due_date: null, linked_subject: null },
      { id: '2', title: 'Low task', priority: 'low', completed: false, due_date: null, linked_subject: null },
    ]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('urgent')).toBeInTheDocument();
    });
    expect(screen.getByText('low')).toBeInTheDocument();
  });

  it('shows linked email subject when present', async () => {
    mockListTasks.mockResolvedValue([
      { id: '1', title: 'Follow up', priority: 'normal', completed: false, due_date: null, linked_subject: 'Re: Q4 Budget', linked_folder: 'INBOX', linked_uid: 42 },
    ]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Re: Q4 Budget')).toBeInTheDocument();
    });
  });

  it('renders delete buttons for each task', async () => {
    mockListTasks.mockResolvedValue([
      { id: '1', title: 'Task one', priority: 'normal', completed: false, due_date: null, linked_subject: null },
      { id: '2', title: 'Task two', priority: 'normal', completed: false, due_date: null, linked_subject: null },
    ]);
    render(<TaskManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Task one')).toBeInTheDocument();
    });
    const deleteButtons = screen.getAllByTitle('Delete task');
    expect(deleteButtons).toHaveLength(2);
  });
});
