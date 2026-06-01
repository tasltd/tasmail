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

describe('apiClient empty-body handling', () => {
  let clientModule: typeof import('./client');
  beforeEach(async () => {
    vi.resetModules();
    clientModule = await import('./client');
  });

  it('returns undefined for 204 No Content', async () => {
    const original = globalThis.fetch;
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(null, { status: 204 }),
    ) as unknown as typeof fetch;
    const result = await clientModule.apiClient.post<void>('/anything');
    expect(result).toBeUndefined();
    globalThis.fetch = original;
  });

  it('returns undefined for 201 Created with empty body (drafts/save flow)', async () => {
    const original = globalThis.fetch;
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response('', { status: 201 }),
    ) as unknown as typeof fetch;
    const result = await clientModule.apiClient.post<void>('/drafts', { foo: 'bar' });
    expect(result).toBeUndefined();
    globalThis.fetch = original;
  });

  it('parses JSON body when present', async () => {
    const original = globalThis.fetch;
    globalThis.fetch = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: 'abc' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    ) as unknown as typeof fetch;
    const result = await clientModule.apiClient.get<{ id: string }>('/anything');
    expect(result).toEqual({ id: 'abc' });
    globalThis.fetch = original;
  });
});

// Added (TMAIL-413): 401 from /auth/login is "wrong credentials" — the client
// must surface an ApiError(401, ...) so LoginPage renders an inline error.
// It MUST NOT try to refresh the token or redirect to /login (which would wipe
// the page's error state before React could render `.login-card__error`).
describe('apiClient 401 handling on auth endpoints', () => {
  let clientModule: typeof import('./client');
  beforeEach(async () => {
    vi.resetModules();
    clientModule = await import('./client');
  });

  it('throws ApiError(401) from POST /auth/login without refresh or redirect', async () => {
    const original = globalThis.fetch;
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: 'Invalid credentials' }), { status: 401 }),
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;
    // Avoid jsdom navigation error if the bug regressed
    const originalLocation = window.location;
    let redirected = false;
    Object.defineProperty(window, 'location', {
      configurable: true,
      value: new Proxy({ href: '/login' } as Location, {
        set: (_t, prop, value) => {
          if (prop === 'href' && value !== '/login') redirected = true;
          return true;
        },
        get: (t, prop) => (t as unknown as Record<string, unknown>)[prop as string],
      }),
    });

    try {
      await clientModule.apiClient.post('/auth/login', { username: 'a', password: 'b' });
      throw new Error('Expected ApiError to be thrown');
    } catch (err) {
      expect(err).toBeInstanceOf(clientModule.ApiError);
      expect((err as InstanceType<typeof clientModule.ApiError>).status).toBe(401);
    }

    // The client must not retry via /auth/refresh on auth endpoints.
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(redirected).toBe(false);
    Object.defineProperty(window, 'location', { configurable: true, value: originalLocation });
    globalThis.fetch = original;
  });

  it('throws ApiError(401) from POST /auth/signup without refresh or redirect', async () => {
    const original = globalThis.fetch;
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ error: 'Email taken' }), { status: 401 }),
    );
    globalThis.fetch = fetchMock as unknown as typeof fetch;

    try {
      await clientModule.apiClient.post('/auth/signup', { email: 'a@b.c', password: 'x' });
      throw new Error('Expected ApiError to be thrown');
    } catch (err) {
      expect(err).toBeInstanceOf(clientModule.ApiError);
      expect((err as InstanceType<typeof clientModule.ApiError>).status).toBe(401);
    }
    expect(fetchMock).toHaveBeenCalledTimes(1);
    globalThis.fetch = original;
  });
});
