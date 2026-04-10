import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ApiError } from './client';

// NOTE: We test ApiError directly and the client behavior via mock fetch
// since apiClient is a singleton that's harder to isolate

describe('ApiError', () => {
  it('stores status and body', () => {
    const err = new ApiError(404, 'Not found');
    expect(err.status).toBe(404);
    expect(err.body).toBe('Not found');
    expect(err.message).toContain('404');
    expect(err.message).toContain('Not found');
  });

  it('is an instance of Error', () => {
    const err = new ApiError(500, 'Server error');
    expect(err).toBeInstanceOf(Error);
  });

  it('has descriptive message format', () => {
    const err = new ApiError(401, 'Unauthorized');
    expect(err.message).toBe('API Error 401: Unauthorized');
  });
});

describe('apiClient token management', () => {
  // Import fresh for each test to test singleton behavior
  let clientModule: typeof import('./client');

  beforeEach(async () => {
    vi.resetModules();
    clientModule = await import('./client');
  });

  it('starts with no token', () => {
    expect(clientModule.apiClient.getToken()).toBeNull();
  });

  it('stores and retrieves token', () => {
    clientModule.apiClient.setToken('test-token');
    expect(clientModule.apiClient.getToken()).toBe('test-token');
  });

  it('clears token when set to null', () => {
    clientModule.apiClient.setToken('test-token');
    clientModule.apiClient.setToken(null);
    expect(clientModule.apiClient.getToken()).toBeNull();
  });
});
