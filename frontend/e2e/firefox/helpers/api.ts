// Added: Authenticated fetch helper for Firefox E2E suite (TMAIL-388).
//
// Satisfies the global SPA-validation rule: every mutation E2E captures
// backend state before AND after the UI action and asserts the resource
// count / payload changed. The thin wrapper here mirrors the runtime
// `ApiClient` semantics (Authorization: Bearer + JSON parsing) but is
// suited for use inside Playwright specs where APIRequestContext is the
// cleanest fetch surface.
import type { APIRequestContext, APIResponse } from '@playwright/test';

export interface ApiClientInit {
  /** Playwright request context — gives baseURL handling + trace integration. */
  request: APIRequestContext;
  /** JWT access token from signup/login. */
  token: string;
}

export class TestApiClient {
  private readonly request: APIRequestContext;
  private readonly token: string;

  constructor({ request, token }: ApiClientInit) {
    this.request = request;
    this.token = token;
  }

  private headers(extra?: Record<string, string>): Record<string, string> {
    return {
      Authorization: `Bearer ${this.token}`,
      'Content-Type': 'application/json',
      ...(extra ?? {}),
    };
  }

  async get<T = unknown>(path: string): Promise<T> {
    const resp = await this.request.get(path, { headers: this.headers() });
    return await parse<T>(resp, 'GET', path);
  }

  async post<T = unknown>(path: string, body?: unknown): Promise<T> {
    const resp = await this.request.post(path, {
      headers: this.headers(),
      data: body ?? {},
    });
    return await parse<T>(resp, 'POST', path);
  }

  async patch<T = unknown>(path: string, body?: unknown): Promise<T> {
    const resp = await this.request.patch(path, {
      headers: this.headers(),
      data: body ?? {},
    });
    return await parse<T>(resp, 'PATCH', path);
  }

  async delete<T = unknown>(path: string): Promise<T> {
    const resp = await this.request.delete(path, { headers: this.headers() });
    return await parse<T>(resp, 'DELETE', path);
  }

  /**
   * Convenience for the before/after pattern. Captures a snapshot of a
   * list endpoint's row count so specs can assert it grew/shrunk by N.
   */
  async count(path: string, arrayKey?: string): Promise<number> {
    const data = await this.get<unknown>(path);
    return extractCount(data, arrayKey);
  }
}

function extractCount(data: unknown, arrayKey?: string): number {
  if (Array.isArray(data)) return data.length;
  if (data && typeof data === 'object') {
    const obj = data as Record<string, unknown>;
    if (arrayKey && Array.isArray(obj[arrayKey])) return (obj[arrayKey] as unknown[]).length;
    // Common shapes returned by TASMail handlers.
    for (const key of ['items', 'data', 'results', 'rows', 'folders', 'messages']) {
      if (Array.isArray(obj[key])) return (obj[key] as unknown[]).length;
    }
    if (typeof obj.total === 'number') return obj.total;
    if (typeof obj.count === 'number') return obj.count;
  }
  throw new Error(`extractCount: response is not a recognised list shape (arrayKey=${arrayKey ?? 'none'})`);
}

async function parse<T>(resp: APIResponse, method: string, path: string): Promise<T> {
  if (!resp.ok()) {
    const text = await resp.text().catch(() => '');
    throw new Error(`${method} ${path} → HTTP ${resp.status()} body=${text.slice(0, 500)}`);
  }
  if (resp.status() === 204) return undefined as unknown as T;
  return (await resp.json()) as T;
}
