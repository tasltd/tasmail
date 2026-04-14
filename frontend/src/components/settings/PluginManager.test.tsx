// Added: PluginManager component tests for TMAIL-132

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PluginManager } from './PluginManager';

const mockListPlugins = vi.fn();
const mockCreatePlugin = vi.fn();
const mockUpdatePlugin = vi.fn();
const mockDeletePlugin = vi.fn();
const mockListExecutions = vi.fn();
const mockTestPlugin = vi.fn();

vi.mock('../../api/plugins', () => ({
  listPlugins: () => mockListPlugins(),
  createPlugin: (...args: unknown[]) => mockCreatePlugin(...args),
  updatePlugin: (...args: unknown[]) => mockUpdatePlugin(...args),
  deletePlugin: (...args: unknown[]) => mockDeletePlugin(...args),
  listExecutions: (...args: unknown[]) => mockListExecutions(...args),
  testPlugin: (...args: unknown[]) => mockTestPlugin(...args),
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

describe('PluginManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Plugins heading after loading', async () => {
    mockListPlugins.mockResolvedValue([]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Plugins')).toBeInTheDocument();
    });
  });

  it('shows empty state when no plugins exist', async () => {
    mockListPlugins.mockResolvedValue([]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No plugins configured. Add one to extend TASMail with custom functionality.'),
      ).toBeInTheDocument();
    });
  });

  it('renders plugin list with name and status', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'Slack Notifier',
        description: 'Posts to Slack',
        plugin_type: 'webhook',
        config: { url: 'https://hooks.slack.com/...' },
        hooks: ['on_receive', 'on_send'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
      {
        id: 'p-2',
        user_id: 'u-1',
        name: 'Spam Filter',
        description: null,
        plugin_type: 'filter',
        config: { rules: [] },
        hooks: ['on_receive'],
        enabled: false,
        created_at: '2026-04-09T12:00:00Z',
        updated_at: '2026-04-09T12:00:00Z',
      },
    ]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Slack Notifier')).toBeInTheDocument();
      expect(screen.getByText('Spam Filter')).toBeInTheDocument();
    });
    expect(screen.getByText('Enabled')).toBeInTheDocument();
    expect(screen.getByText('Disabled')).toBeInTheDocument();
  });

  it('shows add plugin form when Add Plugin is clicked', async () => {
    mockListPlugins.mockResolvedValue([]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Plugin')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Plugin'));

    expect(screen.getByText('New Plugin')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Plugin name')).toBeInTheDocument();
  });

  it('shows hook checkboxes in the create form', async () => {
    mockListPlugins.mockResolvedValue([]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Plugin')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Plugin'));

    expect(screen.getByText('On Receive')).toBeInTheDocument();
    expect(screen.getByText('On Send')).toBeInTheDocument();
    expect(screen.getByText('On Delete')).toBeInTheDocument();
    expect(screen.getByText('On Move')).toBeInTheDocument();
    expect(screen.getByText('On Flag')).toBeInTheDocument();
    expect(screen.getByText('On Read')).toBeInTheDocument();
  });

  it('shows plugin type dropdown in the create form', async () => {
    mockListPlugins.mockResolvedValue([]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Plugin')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Plugin'));

    const select = screen.getByTestId('plugin-type-select');
    expect(select).toBeInTheDocument();
  });

  it('shows execution log when plugin is expanded', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'My Plugin',
        description: null,
        plugin_type: 'webhook',
        config: { url: 'https://example.com' },
        hooks: ['on_receive'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
    ]);
    mockListExecutions.mockResolvedValue([
      {
        id: 'e-1',
        plugin_id: 'p-1',
        event: 'on_receive',
        status: 'success',
        duration_ms: 150,
        error_message: null,
        executed_at: '2026-04-10T12:00:00Z',
      },
    ]);

    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('My Plugin')).toBeInTheDocument();
    });

    // NOTE: Click the expand button
    fireEvent.click(screen.getByTitle('Toggle executions'));

    await waitFor(() => {
      expect(screen.getByTestId('execution-log')).toBeInTheDocument();
    });
  });

  it('shows enable/disable toggle buttons for each plugin', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'Test Plugin',
        description: null,
        plugin_type: 'webhook',
        config: {},
        hooks: ['on_receive'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
    ]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('toggle-p-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Disable')).toBeInTheDocument();
  });

  it('shows test button for each plugin', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'Test Plugin',
        description: null,
        plugin_type: 'webhook',
        config: {},
        hooks: ['on_receive'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
    ]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('test-p-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Test plugin')).toBeInTheDocument();
  });

  it('renders delete buttons for each plugin', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'Plugin 1',
        description: null,
        plugin_type: 'webhook',
        config: {},
        hooks: ['on_send'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
      {
        id: 'p-2',
        user_id: 'u-1',
        name: 'Plugin 2',
        description: null,
        plugin_type: 'filter',
        config: {},
        hooks: ['on_receive'],
        enabled: false,
        created_at: '2026-04-09T12:00:00Z',
        updated_at: '2026-04-09T12:00:00Z',
      },
    ]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
  });

  it('shows plugin type badge for each plugin', async () => {
    mockListPlugins.mockResolvedValue([
      {
        id: 'p-1',
        user_id: 'u-1',
        name: 'Webhook Plugin',
        description: null,
        plugin_type: 'webhook',
        config: {},
        hooks: ['on_receive'],
        enabled: true,
        created_at: '2026-04-10T12:00:00Z',
        updated_at: '2026-04-10T12:00:00Z',
      },
    ]);
    render(<PluginManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('webhook')).toBeInTheDocument();
    });
  });
});
