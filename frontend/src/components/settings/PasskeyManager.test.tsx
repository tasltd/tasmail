// Added: PasskeyManager component tests for TMAIL-83
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { PasskeyManager } from './PasskeyManager';

const mockListCredentials = vi.fn();
const mockRegisterBegin = vi.fn();
const mockRegisterComplete = vi.fn();
const mockDeleteCredential = vi.fn();

vi.mock('../../api/webauthn', () => ({
  webauthnApi: {
    listCredentials: (...args: unknown[]) => mockListCredentials(...args),
    registerBegin: (...args: unknown[]) => mockRegisterBegin(...args),
    registerComplete: (...args: unknown[]) => mockRegisterComplete(...args),
    deleteCredential: (...args: unknown[]) => mockDeleteCredential(...args),
    authenticateBegin: vi.fn(),
    authenticateComplete: vi.fn(),
  },
  bufferToBase64url: vi.fn((_buf: ArrayBuffer) => 'mock-base64url'),
  base64urlToBuffer: vi.fn((_str: string) => new ArrayBuffer(0)),
}));

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  );
}

describe('PasskeyManager', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders "Passkeys (WebAuthn)" heading', async () => {
    mockListCredentials.mockResolvedValue([]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('Passkeys (WebAuthn)')).toBeInTheDocument();
    });
  });

  it('shows "No passkeys registered yet." when list is empty', async () => {
    mockListCredentials.mockResolvedValue([]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('No passkeys registered yet.')).toBeInTheDocument();
    });
  });

  it('renders registered passkeys with name and metadata', async () => {
    mockListCredentials.mockResolvedValue([
      {
        id: 'uuid-1',
        credential_id: 'cred-1',
        name: 'MacBook Touch ID',
        sign_count: 12,
        created_at: '2026-03-01T10:00:00Z',
        last_used_at: '2026-04-10T15:30:00Z',
      },
    ]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText('MacBook Touch ID')).toBeInTheDocument();
    });
    expect(screen.getByText(/Used 12 times/)).toBeInTheDocument();
  });

  it('renders the "Add Passkey" button', async () => {
    mockListCredentials.mockResolvedValue([]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('register-passkey-btn')).toBeInTheDocument();
    });
    expect(screen.getByText('Add Passkey')).toBeInTheDocument();
  });

  it('renders a name input for the new passkey', async () => {
    mockListCredentials.mockResolvedValue([]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('passkey-name-input')).toBeInTheDocument();
    });
  });

  it('renders delete button for each passkey', async () => {
    mockListCredentials.mockResolvedValue([
      {
        id: 'uuid-1',
        credential_id: 'cred-1',
        name: 'YubiKey 5',
        sign_count: 0,
        created_at: '2026-01-01T00:00:00Z',
        last_used_at: null,
      },
    ]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByTestId('delete-passkey-uuid-1')).toBeInTheDocument();
    });
  });

  it('shows description text about passkeys', async () => {
    mockListCredentials.mockResolvedValue([]);
    render(<PasskeyManager />, { wrapper: createWrapper() });

    await waitFor(() => {
      expect(screen.getByText(/biometrics or security key/)).toBeInTheDocument();
    });
  });
});
