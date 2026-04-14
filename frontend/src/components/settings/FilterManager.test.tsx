import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { FilterManager } from './FilterManager';
import * as filtersApi from '../../api/filters';
import type { SieveRule } from '../../api/filters';

vi.mock('../../api/filters');

const mockRules: SieveRule[] = [
  {
    id: 'rule-1',
    mailbox_id: 'mb-1',
    name: 'Move newsletters',
    priority: 0,
    enabled: true,
    conditions: [{ field: 'from', operator: 'contains', value: 'newsletter' }],
    match_mode: 'all',
    actions: [{ action_type: 'move', target: 'Newsletters' }],
    stop_processing: true,
    created_at: '2026-04-10T00:00:00Z',
    updated_at: '2026-04-10T00:00:00Z',
  },
  {
    id: 'rule-2',
    mailbox_id: 'mb-1',
    name: 'Delete spam',
    priority: 1,
    enabled: false,
    conditions: [{ field: 'from', operator: 'equals', value: 'spam@bad.com' }],
    match_mode: 'all',
    actions: [{ action_type: 'delete' }],
    stop_processing: true,
    created_at: '2026-04-10T01:00:00Z',
    updated_at: '2026-04-10T01:00:00Z',
  },
];

function renderWithQueryClient(ui: React.ReactElement) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={qc}>{ui}</QueryClientProvider>);
}

describe('FilterManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(filtersApi.listFilters).mockResolvedValue(mockRules);
  });

  it('renders filter list', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => {
      expect(screen.getByText('Move newsletters')).toBeInTheDocument();
      expect(screen.getByText('Delete spam')).toBeInTheDocument();
    });
  });

  it('shows condition and action counts', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => {
      // Both rules have 1 condition each
      const items = screen.getAllByText(/condition/);
      expect(items.length).toBeGreaterThanOrEqual(2);
    });
  });

  it('renders empty state when no rules', async () => {
    vi.mocked(filtersApi.listFilters).mockResolvedValue([]);
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => {
      expect(screen.getByText(/No filter rules yet/)).toBeInTheDocument();
    });
  });

  it('opens new filter form when clicking New Filter', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    fireEvent.click(screen.getByText('New Filter'));
    expect(screen.getByText('New Filter Rule')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('e.g., Move newsletters')).toBeInTheDocument();
  });

  it('opens edit form when clicking edit button', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    const editButtons = screen.getAllByTitle('Edit');
    fireEvent.click(editButtons[0]);
    expect(screen.getByDisplayValue('Move newsletters')).toBeInTheDocument();
  });

  it('renders delete buttons for each rule', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    const deleteButtons = screen.getAllByTitle('Delete');
    expect(deleteButtons).toHaveLength(2);
  });

  it('shows enable/disable toggle for each rule', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    // First rule is enabled -> shows Disable, second disabled -> shows Enable
    expect(screen.getByTitle('Disable')).toBeInTheDocument();
    expect(screen.getByTitle('Enable')).toBeInTheDocument();
  });

  it('shows priority ordering controls', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    const upButtons = screen.getAllByTitle('Move up');
    const downButtons = screen.getAllByTitle('Move down');
    expect(upButtons).toHaveLength(2);
    expect(downButtons).toHaveLength(2);
    // First rule's up button should be disabled
    expect(upButtons[0]).toBeDisabled();
    // Last rule's down button should be disabled
    expect(downButtons[downButtons.length - 1]).toBeDisabled();
  });

  it('renders back button to return to filters list from editor', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => screen.getByText('Move newsletters'));
    fireEvent.click(screen.getByText('New Filter'));
    expect(screen.getByText('Back to filters')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Back to filters'));
    await waitFor(() => {
      expect(screen.getByText('Move newsletters')).toBeInTheDocument();
    });
  });

  it('renders disabled rule with reduced opacity', async () => {
    renderWithQueryClient(<FilterManager />);
    await waitFor(() => {
      const disabledItem = screen.getByText('Delete spam').closest('.filter-item');
      expect(disabledItem).toHaveStyle({ opacity: '0.5' });
    });
  });
});
