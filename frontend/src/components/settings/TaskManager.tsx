// Added: Task/to-do manager component for TMAIL-126
import React from 'react';
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Plus, Trash2, ArrowLeft, CheckSquare, Square, Mail } from 'lucide-react';
import {
  listTasks,
  createTask,
  updateTask,
  deleteTask,
} from '../../api/tasks';
import type { Task, CreateTaskRequest } from '../../api/tasks';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

// Added: Priority color mapping for visual badges
const PRIORITY_COLORS: Record<string, string> = {
  low: '#6b7280',
  normal: '#3b82f6',
  high: '#f59e0b',
  urgent: '#ef4444',
};

// Added: Filter tab type for task list filtering
type FilterTab = 'all' | 'active' | 'completed';

// Added: Inline form for creating new tasks
function TaskForm({ onSave, onCancel }: { onSave: (data: CreateTaskRequest) => void; onCancel: () => void }) {
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [dueDate, setDueDate] = useState('');
  const [priority, setPriority] = useState('normal');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    onSave({
      title,
      description: description || undefined,
      due_date: dueDate ? new Date(dueDate).toISOString() : undefined,
      priority,
    });
  };

  return (
    <form className="composer__fields" onSubmit={handleSubmit} style={{ gap: '12px' }}>
      <div className="composer__field">
        <label>Title</label>
        <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Task title" required />
      </div>
      <div className="composer__field" style={{ flexDirection: 'column', alignItems: 'flex-start' }}>
        <label>Description</label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Optional description"
          rows={3}
          style={{ width: '100%', padding: '8px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontSize: '13px' }}
        />
      </div>
      <div className="composer__field">
        <label>Due Date</label>
        <input type="datetime-local" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
      </div>
      <div className="composer__field">
        <label>Priority</label>
        <select value={priority} onChange={(e) => setPriority(e.target.value)} style={{ padding: '6px 12px', border: '1px solid var(--color-border)', borderRadius: '6px', fontSize: '13px' }}>
          <option value="low">Low</option>
          <option value="normal">Normal</option>
          <option value="high">High</option>
          <option value="urgent">Urgent</option>
        </select>
      </div>
      <div className="composer__actions">
        <button type="submit" className="btn btn--primary">Add Task</button>
        <button type="button" className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </form>
  );
}

export function TaskManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);
  const [isCreating, setIsCreating] = useState(false);
  const [filterTab, setFilterTab] = useState<FilterTab>('all');

  // NOTE: Fetch tasks based on active filter tab
  const completedFilter = filterTab === 'all' ? undefined : filterTab === 'completed';

  const { data: tasks, isLoading } = useQuery({
    queryKey: ['tasks', filterTab],
    queryFn: () => listTasks(completedFilter),
  });

  const createMut = useMutation({
    mutationFn: createTask,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] });
      setIsCreating(false);
    },
  });

  // Added: Toggle task completion via update mutation
  const toggleMut = useMutation({
    mutationFn: ({ id, completed }: { id: string; completed: boolean }) =>
      updateTask(id, { completed }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tasks'] }),
  });

  const deleteMut = useMutation({
    mutationFn: deleteTask,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['tasks'] }),
  });

  if (isLoading) return <LoadingSkeleton rows={6} />;

  // Added: Format due date for display badge
  const formatDueDate = (dateStr: string | null) => {
    if (!dateStr) return null;
    const date = new Date(dateStr);
    const now = new Date();
    const isOverdue = date < now;
    const formatted = date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    return { formatted, isOverdue };
  };

  return (
    <div style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Tasks</h2>
        <button className="btn btn--primary" onClick={() => setIsCreating(true)}>
          <Plus size={16} /> Add Task
        </button>
      </div>

      {/* Added: Filter tabs for All / Active / Completed */}
      <div style={{ display: 'flex', gap: '4px', margin: '12px 0', borderBottom: '1px solid var(--color-border)', paddingBottom: '8px' }}>
        {(['all', 'active', 'completed'] as FilterTab[]).map((tab) => (
          <button
            key={tab}
            className={`btn ${filterTab === tab ? 'btn--primary' : ''}`}
            onClick={() => setFilterTab(tab)}
            style={{ textTransform: 'capitalize', fontSize: '13px' }}
          >
            {tab}
          </button>
        ))}
      </div>

      {isCreating && (
        <div style={{ marginTop: '12px', padding: '16px', border: '1px solid var(--color-border)', borderRadius: '8px' }}>
          <h3 style={{ marginBottom: '12px' }}>New Task</h3>
          <TaskForm onSave={(data) => createMut.mutate(data)} onCancel={() => setIsCreating(false)} />
        </div>
      )}

      <div style={{ marginTop: '12px' }}>
        {(!tasks || tasks.length === 0) && !isCreating && (
          <p style={{ color: 'var(--color-text-secondary)', textAlign: 'center', padding: '24px' }}>
            {filterTab === 'all'
              ? 'No tasks yet. Add one to get started.'
              : filterTab === 'active'
                ? 'No active tasks.'
                : 'No completed tasks.'}
          </p>
        )}
        {tasks?.map((task: Task) => {
          const dueDateInfo = formatDueDate(task.due_date);
          return (
            <div
              key={task.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '10px 12px',
                borderBottom: '1px solid var(--color-border)',
                opacity: task.completed ? 0.6 : 1,
              }}
            >
              {/* Added: Checkbox toggle for task completion */}
              <button
                className="btn btn--icon"
                onClick={() => toggleMut.mutate({ id: task.id, completed: !task.completed })}
                title={task.completed ? 'Mark incomplete' : 'Mark complete'}
                style={{ flexShrink: 0 }}
              >
                {task.completed ? <CheckSquare size={20} /> : <Square size={20} />}
              </button>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', textDecoration: task.completed ? 'line-through' : 'none' }}>
                  {task.title}
                </div>
                <div style={{ fontSize: '12px', color: 'var(--color-text-secondary)', display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                  {/* Added: Priority badge */}
                  <span style={{
                    padding: '1px 6px',
                    borderRadius: '4px',
                    fontSize: '11px',
                    fontWeight: 600,
                    color: 'white',
                    background: PRIORITY_COLORS[task.priority] || PRIORITY_COLORS.normal,
                  }}>
                    {task.priority}
                  </span>
                  {/* Added: Due date badge with overdue styling */}
                  {dueDateInfo && (
                    <span style={{ color: dueDateInfo.isOverdue && !task.completed ? '#ef4444' : 'inherit' }}>
                      Due {dueDateInfo.formatted}
                    </span>
                  )}
                  {/* Added: Linked email subject indicator */}
                  {task.linked_subject && (
                    <span style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
                      <Mail size={12} />
                      {task.linked_subject}
                    </span>
                  )}
                </div>
              </div>
              <button className="btn btn--icon btn--danger" onClick={() => deleteMut.mutate(task.id)} title="Delete task">
                <Trash2 size={16} />
              </button>
            </div>
          );
        })}
      </div>
    </div>
  );
}
