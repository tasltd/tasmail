// TMAIL-401: preferences API client tests. Asserts the HTTP verbs and the
// path the SPA hits so the backend route in router.rs stays in sync with
// the frontend. Uses fetch mocks rather than MSW for symmetry with the
// neighbouring api/*.test.ts files.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { apiClient } from './client';
import {
  fetchFirstLoginTourSeen,
  markFirstLoginTourSeen,
} from './preferences';

describe('preferences API client', () => {
  beforeEach(() => {
    apiClient.setToken('test-token');
    // @ts-expect-error -- jsdom global fetch is mockable.
    global.fetch = vi.fn();
  });

  afterEach(() => {
    apiClient.setToken(null);
    vi.restoreAllMocks();
  });

  it('GET fetches /me/preferences/first-login-tour-seen and returns the seen flag', async () => {
    const mockResponse = new Response(JSON.stringify({ seen: false }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
    // @ts-expect-error -- typed as vi.Mock
    global.fetch.mockResolvedValue(mockResponse);

    const result = await fetchFirstLoginTourSeen();

    expect(result).toEqual({ seen: false });
    // @ts-expect-error -- typed as vi.Mock
    const [calledUrl, calledOpts] = global.fetch.mock.calls[0];
    expect(calledUrl).toContain('/me/preferences/first-login-tour-seen');
    // ApiClient.get() doesn't set a method (fetch defaults to GET); the
    // important contract is "no method override", not the literal string.
    expect(calledOpts.method).toBeUndefined();
    expect(calledOpts.headers.Authorization).toBe('Bearer test-token');
  });

  it('PATCH marks the tour seen and returns the updated flag', async () => {
    const mockResponse = new Response(JSON.stringify({ seen: true }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
    // @ts-expect-error -- typed as vi.Mock
    global.fetch.mockResolvedValue(mockResponse);

    const result = await markFirstLoginTourSeen();

    expect(result).toEqual({ seen: true });
    // @ts-expect-error -- typed as vi.Mock
    const [calledUrl, calledOpts] = global.fetch.mock.calls[0];
    expect(calledUrl).toContain('/me/preferences/first-login-tour-seen');
    expect(calledOpts.method).toBe('PATCH');
    expect(calledOpts.headers.Authorization).toBe('Bearer test-token');
  });
});
