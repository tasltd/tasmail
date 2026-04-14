// Added: Tests for email tasks API module (TMAIL-126)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { listTasks, createTask, getTask, updateTask, deleteTask } from './tasks';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    get: vi.fn(),
    post: vi.fn(),
    put: vi.fn(),
    delete: vi.fn(),
  },
}));

describe('tasks API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('listTasks', () => {
    it('calls GET /tasks without filter', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      const result = await listTasks();
      expect(apiClient.get).toHaveBeenCalledWith('/tasks');
      expect(result).toEqual([]);
    });

    it('calls GET /tasks?completed=false for active tasks', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listTasks(false);
      expect(apiClient.get).toHaveBeenCalledWith('/tasks?completed=false');
    });

    it('calls GET /tasks?completed=true for completed tasks', async () => {
      vi.mocked(apiClient.get).mockResolvedValue([]);
      await listTasks(true);
      expect(apiClient.get).toHaveBeenCalledWith('/tasks?completed=true');
    });
  });

  describe('createTask', () => {
    it('calls POST /tasks with task data', async () => {
      const taskData = { title: 'Review proposal', priority: 'high' };
      const mockResponse = { id: 'task-123', ...taskData, completed: false };
      vi.mocked(apiClient.post).mockResolvedValue(mockResponse);

      const result = await createTask(taskData);
      expect(apiClient.post).toHaveBeenCalledWith('/tasks', taskData);
      expect(result.title).toBe('Review proposal');
    });
  });

  describe('getTask', () => {
    it('calls GET /tasks/:id', async () => {
      const mockTask = { id: 'task-123', title: 'Test task', completed: false };
      vi.mocked(apiClient.get).mockResolvedValue(mockTask);

      const result = await getTask('task-123');
      expect(apiClient.get).toHaveBeenCalledWith('/tasks/task-123');
      expect(result.title).toBe('Test task');
    });
  });

  describe('updateTask', () => {
    it('calls PUT /tasks/:id with update data', async () => {
      const updateData = { completed: true };
      vi.mocked(apiClient.put).mockResolvedValue({ id: 'task-123', completed: true });

      const result = await updateTask('task-123', updateData);
      expect(apiClient.put).toHaveBeenCalledWith('/tasks/task-123', updateData);
      expect(result.completed).toBe(true);
    });
  });

  describe('deleteTask', () => {
    it('calls DELETE /tasks/:id', async () => {
      vi.mocked(apiClient.delete).mockResolvedValue(undefined);
      await deleteTask('task-123');
      expect(apiClient.delete).toHaveBeenCalledWith('/tasks/task-123');
    });
  });
});
