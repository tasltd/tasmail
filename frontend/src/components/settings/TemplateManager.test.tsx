// Added: Unit tests for TemplateManager component (TMAIL-94)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { TemplateManager } from './TemplateManager';

const mockListTemplates = vi.fn();
const mockCreateTemplate = vi.fn();
const mockUpdateTemplate = vi.fn();
const mockDeleteTemplate = vi.fn();
const mockRenderTemplate = vi.fn();

vi.mock('../../api/templates', () => ({
  listTemplates: () => mockListTemplates(),
  createTemplate: (...args: unknown[]) => mockCreateTemplate(...args),
  updateTemplate: (...args: unknown[]) => mockUpdateTemplate(...args),
  deleteTemplate: (...args: unknown[]) => mockDeleteTemplate(...args),
  renderTemplate: (...args: unknown[]) => mockRenderTemplate(...args),
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

// Added: Sample template fixtures for testing
const sampleTemplates = [
  {
    id: 'tpl-1',
    mailbox_id: 'mb-1',
    name: 'Welcome Email',
    subject: 'Welcome {{name}}!',
    body_html: '<h1>Hello {{name}}</h1>',
    body_text: 'Hello {{name}}',
    merge_fields: ['name', 'email'],
    category: 'Onboarding',
    is_shared: true,
    created_at: '2026-04-10T10:00:00Z',
    updated_at: '2026-04-10T10:00:00Z',
  },
  {
    id: 'tpl-2',
    mailbox_id: 'mb-1',
    name: 'Invoice Reminder',
    subject: 'Invoice #{{invoice_id}} Due',
    body_html: '<p>Your invoice is due.</p>',
    body_text: 'Your invoice is due.',
    merge_fields: ['invoice_id'],
    category: 'Billing',
    is_shared: false,
    created_at: '2026-04-11T10:00:00Z',
    updated_at: '2026-04-11T10:00:00Z',
  },
];

describe('TemplateManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders heading and New Template button after loading', async () => {
    mockListTemplates.mockResolvedValue([]);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Email Templates')).toBeInTheDocument();
    });
    expect(screen.getByText(/New Template/)).toBeInTheDocument();
  });

  it('shows empty state when no templates exist', async () => {
    mockListTemplates.mockResolvedValue([]);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No templates yet. Create one to speed up your email workflow.'),
      ).toBeInTheDocument();
    });
  });

  it('renders template list with names and subjects', async () => {
    mockListTemplates.mockResolvedValue(sampleTemplates);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Welcome Email')).toBeInTheDocument();
      expect(screen.getByText('Invoice Reminder')).toBeInTheDocument();
    });
    // NOTE: Check subject and category display
    expect(screen.getByText(/Welcome \{\{name\}\}!/)).toBeInTheDocument();
    expect(screen.getByText(/Onboarding/)).toBeInTheDocument();
  });

  it('shows Shared badge for shared templates', async () => {
    mockListTemplates.mockResolvedValue(sampleTemplates);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Shared')).toBeInTheDocument();
    });
  });

  it('shows create form when New Template is clicked', async () => {
    mockListTemplates.mockResolvedValue([]);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/New Template/)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/New Template/));

    expect(screen.getByTestId('template-name')).toBeInTheDocument();
    expect(screen.getByTestId('template-subject')).toBeInTheDocument();
    expect(screen.getByTestId('template-body-html')).toBeInTheDocument();
    expect(screen.getByTestId('template-merge-fields')).toBeInTheDocument();
  });

  it('renders edit, preview, and delete buttons for each template', async () => {
    mockListTemplates.mockResolvedValue(sampleTemplates);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('edit-tpl-1')).toBeInTheDocument();
      expect(screen.getByTestId('edit-tpl-2')).toBeInTheDocument();
      expect(screen.getByTestId('preview-tpl-1')).toBeInTheDocument();
      expect(screen.getByTestId('delete-tpl-1')).toBeInTheDocument();
      expect(screen.getByTestId('delete-tpl-2')).toBeInTheDocument();
    });
  });

  it('shows merge field count in template details', async () => {
    mockListTemplates.mockResolvedValue(sampleTemplates);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/2 merge fields/)).toBeInTheDocument();
      expect(screen.getByText(/1 merge field$/)).toBeInTheDocument();
    });
  });

  it('navigates back when back button is clicked', async () => {
    mockListTemplates.mockResolvedValue([]);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTitle('Back')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTitle('Back'));
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('shows preview panel when preview button is clicked', async () => {
    mockListTemplates.mockResolvedValue(sampleTemplates);
    render(<TemplateManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('preview-tpl-1')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('preview-tpl-1'));

    expect(screen.getByText('Preview: Welcome Email')).toBeInTheDocument();
    expect(screen.getByTestId('preview-field-name')).toBeInTheDocument();
    expect(screen.getByTestId('preview-field-email')).toBeInTheDocument();
    expect(screen.getByText('Render Preview')).toBeInTheDocument();
  });
});
