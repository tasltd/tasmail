// Added: ChatIntegrationManager component tests for TMAIL-129

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { ChatIntegrationManager } from './ChatIntegrationManager';

const mockListChatIntegrations = vi.fn();
const mockCreateChatIntegration = vi.fn();
const mockUpdateChatIntegration = vi.fn();
const mockDeleteChatIntegration = vi.fn();
const mockTestChatIntegration = vi.fn();

vi.mock('../../api/chat-integrations', () => ({
  listChatIntegrations: () => mockListChatIntegrations(),
  createChatIntegration: (...args: unknown[]) => mockCreateChatIntegration(...args),
  updateChatIntegration: (...args: unknown[]) => mockUpdateChatIntegration(...args),
  deleteChatIntegration: (...args: unknown[]) => mockDeleteChatIntegration(...args),
  testChatIntegration: (...args: unknown[]) => mockTestChatIntegration(...args),
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

describe('ChatIntegrationManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Chat Integrations heading after loading', async () => {
    mockListChatIntegrations.mockResolvedValue([]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Chat Integrations')).toBeInTheDocument();
    });
  });

  it('shows empty state when no integrations exist', async () => {
    mockListChatIntegrations.mockResolvedValue([]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No chat integrations configured. Add one to forward email notifications to your team chat.'),
      ).toBeInTheDocument();
    });
  });

  it('renders integration list with platform badge and active status', async () => {
    mockListChatIntegrations.mockResolvedValue([
      {
        id: 'ci-1',
        platform: 'slack',
        webhook_url: 'https://hooks.slack.com/services/T00/B00/xxx',
        channel_name: '#general',
        notify_on_receive: true,
        notify_on_send: false,
        notify_on_mention: true,
        filter_from: null,
        filter_subject: null,
        active: true,
      },
      {
        id: 'ci-2',
        platform: 'discord',
        webhook_url: 'https://discord.com/api/webhooks/123/abc',
        channel_name: null,
        notify_on_receive: false,
        notify_on_send: true,
        notify_on_mention: false,
        filter_from: null,
        filter_subject: null,
        active: false,
      },
    ]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Slack')).toBeInTheDocument();
      expect(screen.getByText('Discord')).toBeInTheDocument();
    });
    expect(screen.getByText('#general')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
  });

  it('shows add form when Add Integration is clicked', async () => {
    mockListChatIntegrations.mockResolvedValue([]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Integration')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Integration'));

    expect(screen.getByText('New Chat Integration')).toBeInTheDocument();
    expect(screen.getByTestId('platform-select')).toBeInTheDocument();
    expect(screen.getByTestId('webhook-url-input')).toBeInTheDocument();
  });

  it('shows platform selector with all platform options', async () => {
    mockListChatIntegrations.mockResolvedValue([]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Integration')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Integration'));

    const platformSelect = screen.getByTestId('platform-select');
    // NOTE: Verify all platform options are present
    expect(platformSelect).toBeInTheDocument();
    const options = platformSelect.querySelectorAll('option');
    expect(options.length).toBe(5);
    expect(options[0].textContent).toBe('Slack');
    expect(options[1].textContent).toBe('Microsoft Teams');
    expect(options[2].textContent).toBe('Google Chat');
    expect(options[3].textContent).toBe('Discord');
    expect(options[4].textContent).toBe('Custom');
  });

  it('shows notification toggle checkboxes in the form', async () => {
    mockListChatIntegrations.mockResolvedValue([]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Integration')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Integration'));

    // Added: Verify notification toggles are present with correct defaults
    const receiveCheckbox = screen.getByTestId('notify-receive') as HTMLInputElement;
    const sendCheckbox = screen.getByTestId('notify-send') as HTMLInputElement;
    const mentionCheckbox = screen.getByTestId('notify-mention') as HTMLInputElement;

    expect(receiveCheckbox.checked).toBe(true);
    expect(sendCheckbox.checked).toBe(false);
    expect(mentionCheckbox.checked).toBe(true);
  });

  it('shows test button for each integration', async () => {
    mockListChatIntegrations.mockResolvedValue([
      {
        id: 'ci-1',
        platform: 'slack',
        webhook_url: 'https://hooks.slack.com/services/T00/B00/xxx',
        channel_name: '#general',
        notify_on_receive: true,
        notify_on_send: false,
        notify_on_mention: true,
        filter_from: null,
        filter_subject: null,
        active: true,
      },
    ]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('test-ci-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Send test notification')).toBeInTheDocument();
  });

  it('renders delete buttons for each integration', async () => {
    mockListChatIntegrations.mockResolvedValue([
      {
        id: 'ci-1',
        platform: 'slack',
        webhook_url: 'https://hooks.slack.com/services/T00/B00/xxx',
        channel_name: null,
        notify_on_receive: true,
        notify_on_send: false,
        notify_on_mention: true,
        filter_from: null,
        filter_subject: null,
        active: true,
      },
      {
        id: 'ci-2',
        platform: 'teams',
        webhook_url: 'https://outlook.office.com/webhook/xxx',
        channel_name: null,
        notify_on_receive: true,
        notify_on_send: false,
        notify_on_mention: true,
        filter_from: null,
        filter_subject: null,
        active: false,
      },
    ]);
    render(<ChatIntegrationManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
  });
});
