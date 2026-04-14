// Added: Tests for SmartReplyBar component (TMAIL-104)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { SmartReplyBar } from './SmartReplyBar';

// Added: Mock the AI config API for smart reply
const mockGetSmartReply = vi.fn();

vi.mock('../../api/ai-config', () => ({
  getSmartReply: (...args: unknown[]) => mockGetSmartReply(...args),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return function Wrapper({ children }: { children: React.ReactNode }) {
    return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
  };
}

describe('SmartReplyBar', () => {
  const defaultProps = {
    folder: 'INBOX',
    uid: 42,
    onUseReply: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders all three tone buttons', () => {
    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    expect(screen.getByTestId('smart-reply-tone-brief')).toBeInTheDocument();
    expect(screen.getByTestId('smart-reply-tone-detailed')).toBeInTheDocument();
    expect(screen.getByTestId('smart-reply-tone-decline')).toBeInTheDocument();
    expect(screen.getByText('Brief')).toBeInTheDocument();
    expect(screen.getByText('Detailed')).toBeInTheDocument();
    expect(screen.getByText('Decline')).toBeInTheDocument();
  });

  it('shows loading state while generating reply', async () => {
    // Added: Create a promise that won't resolve immediately to simulate loading
    mockGetSmartReply.mockReturnValue(new Promise(() => {}));

    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('smart-reply-tone-brief'));

    await waitFor(() => {
      expect(screen.getByTestId('smart-reply-loading')).toBeInTheDocument();
    });
    expect(screen.getByText('Generating reply...')).toBeInTheDocument();
  });

  it('shows generated reply in textarea after successful generation', async () => {
    mockGetSmartReply.mockResolvedValue({
      reply: 'Thank you for your email. I will review the report.',
      tone: 'brief',
      provider: 'openai',
      model: 'gpt-4o',
    });

    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('smart-reply-tone-brief'));

    await waitFor(() => {
      expect(screen.getByTestId('smart-reply-result')).toBeInTheDocument();
    });
    const replyTextarea = screen.getByTestId('smart-reply-textarea') as HTMLTextAreaElement;
    expect(replyTextarea.value).toBe('Thank you for your email. I will review the report.');
  });

  it('shows "Use this reply" button after reply is generated', async () => {
    mockGetSmartReply.mockResolvedValue({
      reply: 'Thank you for letting me know.',
      tone: 'brief',
      provider: 'openai',
      model: 'gpt-4o',
    });

    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('smart-reply-tone-brief'));

    await waitFor(() => {
      expect(screen.getByTestId('smart-reply-use-btn')).toBeInTheDocument();
    });
    expect(screen.getByText('Use this reply')).toBeInTheDocument();
  });

  it('shows regenerate button after reply is generated', async () => {
    mockGetSmartReply.mockResolvedValue({
      reply: 'I appreciate the update.',
      tone: 'detailed',
      provider: 'anthropic',
      model: 'claude-sonnet-4-20250514',
    });

    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    fireEvent.click(screen.getByTestId('smart-reply-tone-detailed'));

    await waitFor(() => {
      expect(screen.getByTestId('smart-reply-regenerate-btn')).toBeInTheDocument();
    });
    expect(screen.getByText('Regenerate')).toBeInTheDocument();
  });

  it('renders in compact form with header and tone buttons visible', () => {
    // Added: Verify initial compact rendering — no result area, no loading
    render(<SmartReplyBar {...defaultProps} />, { wrapper: createWrapper() });

    expect(screen.getByTestId('smart-reply-bar')).toBeInTheDocument();
    expect(screen.getByTestId('smart-reply-tones')).toBeInTheDocument();
    expect(screen.getByText('Smart Reply')).toBeInTheDocument();
    // NOTE: No result or loading areas should exist before any interaction
    expect(screen.queryByTestId('smart-reply-result')).not.toBeInTheDocument();
    expect(screen.queryByTestId('smart-reply-loading')).not.toBeInTheDocument();
  });
});
