// Added: Tests for EmailSummary component (TMAIL-103)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { EmailSummary } from './EmailSummary';

// Added: Mock the AI config API
const mockSummarizeEmail = vi.fn();
const mockSummarizeThread = vi.fn();

vi.mock('../../api/ai-config', () => ({
  summarizeEmail: (...args: unknown[]) => mockSummarizeEmail(...args),
  summarizeThread: (...args: unknown[]) => mockSummarizeThread(...args),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

const defaultProps = {
  folder: 'INBOX',
  uid: 42,
  emailText: 'Hello, please review the quarterly report attached.',
};

describe('EmailSummary', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the summarize button', () => {
    render(<EmailSummary {...defaultProps} />, { wrapper: createWrapper() });
    expect(screen.getByTestId('email-summary-btn')).toBeInTheDocument();
    expect(screen.getByTestId('email-summary-btn')).toHaveTextContent('Summarize');
  });

  it('renders sparkles icon in the summarize button', () => {
    render(<EmailSummary {...defaultProps} />, { wrapper: createWrapper() });
    // NOTE: The Sparkles icon renders as an SVG inside the button
    const summarizeButton = screen.getByTestId('email-summary-btn');
    expect(summarizeButton.querySelector('svg')).toBeInTheDocument();
  });

  it('shows loading state when summarize is clicked', async () => {
    // Added: Mock a delayed response to observe loading state
    mockSummarizeEmail.mockReturnValue(new Promise(() => {}));
    render(<EmailSummary {...defaultProps} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('email-summary-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('email-summary-loading')).toBeInTheDocument();
    });
    expect(screen.getByText('Generating summary...')).toBeInTheDocument();
  });

  it('shows summary card after successful summarization', async () => {
    mockSummarizeEmail.mockResolvedValue({
      summary: 'This email requests a review of the quarterly report.',
      provider: 'openai',
      model: 'gpt-4o',
    });

    render(<EmailSummary {...defaultProps} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByTestId('email-summary-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('email-summary-card')).toBeInTheDocument();
    });
    expect(screen.getByTestId('email-summary-text')).toHaveTextContent(
      'This email requests a review of the quarterly report.',
    );
  });

  it('shows thread summary button when threadUids has multiple messages', () => {
    render(
      <EmailSummary {...defaultProps} threadUids={[40, 41, 42]} />,
      { wrapper: createWrapper() },
    );
    expect(screen.getByTestId('email-summary-thread-btn')).toBeInTheDocument();
    expect(screen.getByTestId('email-summary-thread-btn')).toHaveTextContent(
      'Summarize Thread (3 messages)',
    );
  });

  it('does not show thread button when threadUids has fewer than 2 messages', () => {
    render(
      <EmailSummary {...defaultProps} threadUids={[42]} />,
      { wrapper: createWrapper() },
    );
    expect(screen.queryByTestId('email-summary-thread-btn')).not.toBeInTheDocument();
  });

  it('shows dismiss button and hides summary when dismissed', async () => {
    mockSummarizeEmail.mockResolvedValue({
      summary: 'Summary text here.',
      provider: 'openai',
      model: 'gpt-4o',
    });

    render(<EmailSummary {...defaultProps} />, { wrapper: createWrapper() });
    fireEvent.click(screen.getByTestId('email-summary-btn'));

    await waitFor(() => {
      expect(screen.getByTestId('email-summary-dismiss')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('email-summary-dismiss'));

    // NOTE: After dismissal, the summary card should be gone and buttons should reappear
    expect(screen.queryByTestId('email-summary-card')).not.toBeInTheDocument();
    expect(screen.getByTestId('email-summary-btn')).toBeInTheDocument();
  });
});
