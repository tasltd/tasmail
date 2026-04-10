import { describe, it, expect, vi, beforeEach } from 'vitest';
import { login, logout, restoreSession } from './auth';
import { apiClient } from './client';

vi.mock('./client', () => ({
  apiClient: {
    post: vi.fn(),
    setToken: vi.fn(),
    getToken: vi.fn(),
  },
}));

describe('auth API', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
  });

  describe('login', () => {
    it('stores tokens in localStorage and sets apiClient token', async () => {
      const mockTokens = {
        access_token: 'test-access-token',
        refresh_token: 'test-refresh-token',
        expires_in: 900,
      };
      vi.mocked(apiClient.post).mockResolvedValue(mockTokens);

      const result = await login({ username: 'user@example.com', password: 'pass123' });

      expect(apiClient.post).toHaveBeenCalledWith('/auth/login', {
        username: 'user@example.com',
        password: 'pass123',
      });
      expect(apiClient.setToken).toHaveBeenCalledWith('test-access-token');
      expect(localStorage.getItem('access_token')).toBe('test-access-token');
      expect(localStorage.getItem('refresh_token')).toBe('test-refresh-token');
      expect(result).toEqual(mockTokens);
    });

    it('propagates errors from API', async () => {
      vi.mocked(apiClient.post).mockRejectedValue(new Error('Invalid credentials'));

      await expect(login({ username: 'bad', password: 'bad' })).rejects.toThrow('Invalid credentials');
    });
  });

  describe('logout', () => {
    it('clears tokens from localStorage and apiClient', async () => {
      localStorage.setItem('access_token', 'token');
      localStorage.setItem('refresh_token', 'refresh');
      vi.mocked(apiClient.post).mockResolvedValue(undefined);

      await logout();

      expect(apiClient.post).toHaveBeenCalledWith('/auth/logout');
      expect(apiClient.setToken).toHaveBeenCalledWith(null);
      expect(localStorage.getItem('access_token')).toBeNull();
      expect(localStorage.getItem('refresh_token')).toBeNull();
    });

    it('clears tokens even if API call fails', async () => {
      localStorage.setItem('access_token', 'token');
      vi.mocked(apiClient.post).mockRejectedValue(new Error('Network error'));

      // logout uses try/finally — the error propagates but tokens are still cleared
      try {
        await logout();
      } catch {
        // Expected: the network error propagates
      }

      expect(apiClient.setToken).toHaveBeenCalledWith(null);
      expect(localStorage.getItem('access_token')).toBeNull();
    });
  });

  describe('restoreSession', () => {
    it('returns true and sets token when access_token exists', () => {
      localStorage.setItem('access_token', 'stored-token');

      const result = restoreSession();

      expect(result).toBe(true);
      expect(apiClient.setToken).toHaveBeenCalledWith('stored-token');
    });

    it('returns false when no access_token exists', () => {
      const result = restoreSession();

      expect(result).toBe(false);
      expect(apiClient.setToken).not.toHaveBeenCalled();
    });
  });
});
