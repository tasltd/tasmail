import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAuth } from './useAuth';

const mockLogin = vi.fn();
const mockLogout = vi.fn();
const mockRestoreSession = vi.fn();

vi.mock('../api/auth', () => ({
  login: (...args: unknown[]) => mockLogin(...args),
  logout: (...args: unknown[]) => mockLogout(...args),
  restoreSession: () => mockRestoreSession(),
}));

describe('useAuth', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockRestoreSession.mockReturnValue(false);
  });

  it('starts with isLoading true', () => {
    const { result } = renderHook(() => useAuth());
    // After initial render + effect, isLoading becomes false
    expect(result.current.isAuthenticated).toBe(false);
  });

  it('restores session on mount when token exists', async () => {
    mockRestoreSession.mockReturnValue(true);

    const { result } = renderHook(() => useAuth());

    expect(result.current.isAuthenticated).toBe(true);
    expect(result.current.isLoading).toBe(false);
  });

  it('sets isAuthenticated false when no stored token', () => {
    mockRestoreSession.mockReturnValue(false);

    const { result } = renderHook(() => useAuth());

    expect(result.current.isAuthenticated).toBe(false);
    expect(result.current.isLoading).toBe(false);
  });

  it('login sets isAuthenticated to true', async () => {
    mockLogin.mockResolvedValue({ access_token: 'tok', refresh_token: 'ref', expires_in: 900 });

    const { result } = renderHook(() => useAuth());

    await act(async () => {
      await result.current.login({ username: 'user@test.com', password: 'pass123' });
    });

    expect(result.current.isAuthenticated).toBe(true);
    expect(mockLogin).toHaveBeenCalledWith({ username: 'user@test.com', password: 'pass123' });
  });

  it('login propagates errors', async () => {
    mockLogin.mockRejectedValue(new Error('Invalid credentials'));

    const { result } = renderHook(() => useAuth());

    await expect(
      act(async () => {
        await result.current.login({ username: 'bad', password: 'bad' });
      }),
    ).rejects.toThrow('Invalid credentials');

    expect(result.current.isAuthenticated).toBe(false);
  });

  it('logout sets isAuthenticated to false', async () => {
    mockRestoreSession.mockReturnValue(true);
    mockLogout.mockResolvedValue(undefined);

    const { result } = renderHook(() => useAuth());

    expect(result.current.isAuthenticated).toBe(true);

    await act(async () => {
      await result.current.logout();
    });

    expect(result.current.isAuthenticated).toBe(false);
    expect(mockLogout).toHaveBeenCalled();
  });
});
