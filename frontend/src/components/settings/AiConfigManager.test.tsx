// Added: AiConfigManager component tests for TMAIL-105

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AiConfigManager } from './AiConfigManager';

const mockListAiConfigs = vi.fn();
const mockCreateAiConfig = vi.fn();
const mockUpdateAiConfig = vi.fn();
const mockDeleteAiConfig = vi.fn();
const mockTestAiConfig = vi.fn();

vi.mock('../../api/ai-config', () => ({
  listAiConfigs: () => mockListAiConfigs(),
  createAiConfig: (...args: unknown[]) => mockCreateAiConfig(...args),
  updateAiConfig: (...args: unknown[]) => mockUpdateAiConfig(...args),
  deleteAiConfig: (...args: unknown[]) => mockDeleteAiConfig(...args),
  testAiConfig: (...args: unknown[]) => mockTestAiConfig(...args),
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

describe('AiConfigManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders AI Configuration heading after loading', async () => {
    mockListAiConfigs.mockResolvedValue([]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('AI Configuration')).toBeInTheDocument();
    });
  });

  it('shows empty state when no configs exist', async () => {
    mockListAiConfigs.mockResolvedValue([]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(
        screen.getByText('No AI providers configured. Add one to enable email summarization and smart replies.'),
      ).toBeInTheDocument();
    });
  });

  it('shows provider selector with all provider options when creating', async () => {
    mockListAiConfigs.mockResolvedValue([]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    const providerSelect = screen.getByTestId('provider-select');
    expect(providerSelect).toBeInTheDocument();
    const options = providerSelect.querySelectorAll('option');
    expect(options.length).toBe(5);
    expect(options[0].textContent).toBe('OpenAI');
    expect(options[1].textContent).toBe('Anthropic');
    expect(options[2].textContent).toBe('Google');
    expect(options[3].textContent).toBe('Ollama');
    expect(options[4].textContent).toBe('Custom');
  });

  it('shows API key input as password field', async () => {
    mockListAiConfigs.mockResolvedValue([]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    const apiKeyInput = screen.getByTestId('api-key-input') as HTMLInputElement;
    expect(apiKeyInput).toBeInTheDocument();
    expect(apiKeyInput.type).toBe('password');
  });

  it('shows model suggestions based on selected provider', async () => {
    mockListAiConfigs.mockResolvedValue([]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Add Provider')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText('Add Provider'));

    // NOTE: Default provider is OpenAI, so model should default to gpt-4o
    const modelInput = screen.getByTestId('model-name-input') as HTMLInputElement;
    expect(modelInput.value).toBe('gpt-4o');
  });

  it('shows test button for each configuration', async () => {
    mockListAiConfigs.mockResolvedValue([
      {
        id: 'ai-1',
        provider: 'openai',
        api_key_masked: 'sk-t...2345',
        model_name: 'gpt-4o',
        base_url: null,
        max_tokens: 500,
        temperature: 0.7,
        active: true,
      },
    ]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('test-ai-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Test connection')).toBeInTheDocument();
  });

  it('shows active toggle for each configuration', async () => {
    mockListAiConfigs.mockResolvedValue([
      {
        id: 'ai-1',
        provider: 'anthropic',
        api_key_masked: 'sk-a...wxyz',
        model_name: 'claude-sonnet-4-20250514',
        base_url: null,
        max_tokens: 1000,
        temperature: 0.5,
        active: true,
      },
    ]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('toggle-ai-1')).toBeInTheDocument();
    });
    expect(screen.getByTitle('Deactivate')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
  });

  it('renders config list with provider badge, model, and masked key', async () => {
    mockListAiConfigs.mockResolvedValue([
      {
        id: 'ai-1',
        provider: 'openai',
        api_key_masked: 'sk-t...2345',
        model_name: 'gpt-4o',
        base_url: null,
        max_tokens: 500,
        temperature: 0.7,
        active: true,
      },
      {
        id: 'ai-2',
        provider: 'ollama',
        api_key_masked: '****',
        model_name: 'llama3',
        base_url: 'http://localhost:11434',
        max_tokens: 300,
        temperature: 0.8,
        active: false,
      },
    ]);
    render(<AiConfigManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('OpenAI')).toBeInTheDocument();
      expect(screen.getByText('Ollama')).toBeInTheDocument();
    });
    expect(screen.getByText('gpt-4o')).toBeInTheDocument();
    expect(screen.getByText('llama3')).toBeInTheDocument();
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.getByText('Inactive')).toBeInTheDocument();
  });
});
