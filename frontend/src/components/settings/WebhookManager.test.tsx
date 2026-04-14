// Added: WebhookManager component tests for TMAIL-131

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { WebhookManager } from './WebhookManager';

const mockListWebhooks = vi.fn();
const mockCreateWebhook = vi.fn();
const mockUpdateWebhook = vi.fn();
const mockDeleteWebhook = vi.fn();
const mockListDeliveries = vi.fn();

vi.mock('../../api/webhooks', () => ({
  listWebhooks: () => mockListWebhooks(),
  createWebhook: (...args: unknown[]) => mockCreateWebhook(...args),
  updateWebhook: (...args: unknown[]) => mockUpdateWebhook(...args),
  deleteWebhook: (...args: unknown[]) => mockDeleteWebhook(...args),
  listDeliveries: (...args: unknown[]) => mockListDeliveries(...args),
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

describe('WebhookManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders Webhooks heading after loading', async () => {
    mockListWebhooks.mockResolvedValue([]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Webhooks')).toBeInTheDocument();
    });
  });

  it('shows empty state when no webhooks exist', async () => {
    mockListWebhooks.mockResolvedValue([]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No webhooks configured. Add one to receive notifications for email events.'),
      ).toBeInTheDocument();
    });
  });

  it('renders webhook list with URL and status', async () => {
    mockListWebhooks.mockResolvedValue([
      {
        id: 'wh-1',
        url: 'https://example.com/hook',
        secret: 'secret123',
        events: ['email.received', 'email.sent'],
        active: true,
        description: 'Test webhook',
        last_triggered_at: '2026-04-10T12:00:00Z',
        failure_count: 0,
      },
      {
        id: 'wh-2',
        url: 'https://example.com/hook2',
        secret: 'secret456',
        events: ['email.deleted'],
        active: false,
        description: null,
        last_triggered_at: null,
        failure_count: 5,
      },
    ]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('https://example.com/hook')).toBeInTheDocument();
      expect(screen.getByText('https://example.com/hook2')).toBeInTheDocument();
    });
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
  });

  it('shows add webhook form when Add Webhook is clicked', async () => {
    mockListWebhooks.mockResolvedValue([]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Webhook')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Webhook'));

    expect(screen.getByText('New Webhook')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('https://example.com/webhook')).toBeInTheDocument();
  });

  it('shows event checkboxes in the create form', async () => {
    mockListWebhooks.mockResolvedValue([]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Webhook')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Webhook'));

    expect(screen.getByText('Email Received')).toBeInTheDocument();
    expect(screen.getByText('Email Sent')).toBeInTheDocument();
    expect(screen.getByText('Email Deleted')).toBeInTheDocument();
    expect(screen.getByText('Email Moved')).toBeInTheDocument();
    expect(screen.getByText('Email Flagged')).toBeInTheDocument();
  });

  it('shows delivery log when webhook is expanded', async () => {
    mockListWebhooks.mockResolvedValue([
      {
        id: 'wh-1',
        url: 'https://example.com/hook',
        secret: 'secret123',
        events: ['email.received'],
        active: true,
        description: null,
        last_triggered_at: null,
        failure_count: 0,
      },
    ]);
    mockListDeliveries.mockResolvedValue([
      {
        id: 'del-1',
        webhook_id: 'wh-1',
        event: 'email.received',
        payload: { subject: 'Test' },
        response_status: 200,
        response_body: 'OK',
        delivered_at: '2026-04-10T12:00:00Z',
        success: true,
      },
    ]);

    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('https://example.com/hook')).toBeInTheDocument();
    });

    // NOTE: Click the expand button (first toggle deliveries button)
    fireEvent.click(screen.getByTitle('Toggle deliveries'));

    await waitFor(() => {
      expect(screen.getByTestId('delivery-log')).toBeInTheDocument();
    });
  });

  it('shows active toggle buttons for each webhook', async () => {
    mockListWebhooks.mockResolvedValue([
      {
        id: 'wh-1',
        url: 'https://example.com/hook',
        secret: 'secret123',
        events: ['email.received'],
        active: true,
        description: null,
        last_triggered_at: null,
        failure_count: 0,
      },
    ]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('toggle-wh-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Deactivate')).toBeInTheDocument();
  });

  it('renders delete buttons for each webhook', async () => {
    mockListWebhooks.mockResolvedValue([
      {
        id: 'wh-1',
        url: 'https://example.com/hook1',
        secret: 'secret',
        events: ['email.sent'],
        active: true,
        description: null,
        last_triggered_at: null,
        failure_count: 0,
      },
      {
        id: 'wh-2',
        url: 'https://example.com/hook2',
        secret: 'secret',
        events: ['email.deleted'],
        active: false,
        description: null,
        last_triggered_at: null,
        failure_count: 0,
      },
    ]);
    render(<WebhookManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      const deleteButtons = screen.getAllByTitle('Delete');
      expect(deleteButtons).toHaveLength(2);
    });
  });
});
