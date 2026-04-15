// Added: OllamaManager component tests for TMAIL-102

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { OllamaManager } from './OllamaManager';
import * as ollamaApi from '../../api/ollama';

// Added: Mock the Ollama API module
vi.mock('../../api/ollama', () => ({
  getOllamaConfig: vi.fn(),
  updateOllamaConfig: vi.fn(),
  getOllamaStatus: vi.fn(),
  pullOllamaModel: vi.fn(),
  deleteOllamaModel: vi.fn(),
  listCachedModels: vi.fn(),
  formatModelSize: vi.fn((bytes: number | null) => (bytes ? `${(bytes / 1e9).toFixed(1)} GB` : '—')),
}));

// Added: Mock the mail store
vi.mock('../../stores/mailStore', () => ({
  useMailStore: vi.fn((selector) =>
    selector({
      viewMode: 'ollama',
      setViewMode: vi.fn(),
    }),
  ),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
    },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('OllamaManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Added: Default mock responses for queries
    vi.mocked(ollamaApi.getOllamaConfig).mockResolvedValue({
      id: 'cfg-1',
      base_url: 'http://localhost:11434',
      enabled: false,
      default_model: 'llama3.2',
      max_context_length: 4096,
      gpu_layers: -1,
      updated_at: '2024-01-01T00:00:00Z',
    });
    vi.mocked(ollamaApi.getOllamaStatus).mockResolvedValue({
      running: true,
      version: '0.3.14',
      models: [
        {
          name: 'llama3.2',
          size: 4100000000,
          parameter_size: '8B',
          quantization_level: 'Q4_0',
          modified_at: '2024-01-01T00:00:00Z',
        },
      ],
    });
    vi.mocked(ollamaApi.listCachedModels).mockResolvedValue([]);
  });

  it('renders the heading', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByText('Ollama LLM Server')).toBeInTheDocument();
  });

  it('shows server status when running', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByTestId('running-status')).toHaveTextContent('Running');
    expect(await screen.findByTestId('version')).toHaveTextContent('0.3.14');
  });

  it('shows not running status', async () => {
    vi.mocked(ollamaApi.getOllamaStatus).mockResolvedValue({
      running: false,
      version: null,
      models: [],
    });
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByTestId('running-status')).toHaveTextContent('Not running');
  });

  it('renders config form with loaded values', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    const baseUrlInput = await screen.findByTestId('base-url-input');
    expect(baseUrlInput).toHaveValue('http://localhost:11434');
  });

  it('renders model list when models exist', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByTestId('model-llama3.2')).toBeInTheDocument();
  });

  it('renders pull model section', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByTestId('pull-model-input')).toBeInTheDocument();
    expect(await screen.findByTestId('pull-model-btn')).toBeInTheDocument();
  });

  it('disables pull button when input is empty', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    const pullBtn = await screen.findByTestId('pull-model-btn');
    expect(pullBtn).toBeDisabled();
  });

  it('enables pull button when model name is entered', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    const input = await screen.findByTestId('pull-model-input');
    fireEvent.change(input, { target: { value: 'mistral' } });
    const pullBtn = screen.getByTestId('pull-model-btn');
    expect(pullBtn).not.toBeDisabled();
  });

  it('renders the refresh button', async () => {
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByTestId('refresh-status')).toBeInTheDocument();
  });

  it('shows empty state when no models and server running', async () => {
    vi.mocked(ollamaApi.getOllamaStatus).mockResolvedValue({
      running: true,
      version: '0.3.14',
      models: [],
    });
    render(<OllamaManager />, { wrapper: createWrapper() });
    expect(await screen.findByText(/No models installed/)).toBeInTheDocument();
  });
});
