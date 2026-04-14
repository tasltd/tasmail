// Added: Tasks API module for TMAIL-126 — email-linked task/to-do management
import { apiClient } from './client';

// Added: Task interface matching backend EmailTask struct
export interface Task {
  id: string;
  user_id: string;
  title: string;
  description: string | null;
  due_date: string | null;
  completed: boolean;
  completed_at: string | null;
  priority: string;
  linked_folder: string | null;
  linked_uid: number | null;
  linked_subject: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateTaskRequest {
  title: string;
  description?: string;
  due_date?: string;
  priority?: string;
  linked_folder?: string;
  linked_uid?: number;
  linked_subject?: string;
}

export interface UpdateTaskRequest {
  title?: string;
  description?: string;
  due_date?: string;
  priority?: string;
  completed?: boolean;
  linked_folder?: string;
  linked_uid?: number;
  linked_subject?: string;
}

/// PURPOSE: List user tasks with optional completion filter
export async function listTasks(completed?: boolean): Promise<Task[]> {
  const params = completed !== undefined ? `?completed=${completed}` : '';
  return apiClient.get<Task[]>(`/tasks${params}`);
}

/// PURPOSE: Create a new task
export async function createTask(data: CreateTaskRequest): Promise<Task> {
  return apiClient.post<Task>('/tasks', data);
}

/// PURPOSE: Get a single task by ID
export async function getTask(id: string): Promise<Task> {
  return apiClient.get<Task>(`/tasks/${id}`);
}

/// PURPOSE: Update an existing task
export async function updateTask(id: string, data: UpdateTaskRequest): Promise<Task> {
  return apiClient.put<Task>(`/tasks/${id}`, data);
}

/// PURPOSE: Delete a task
export async function deleteTask(id: string): Promise<void> {
  await apiClient.delete(`/tasks/${id}`);
}
