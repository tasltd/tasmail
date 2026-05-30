import { API_BASE_URL } from './constants';

class ApiClient {
  private accessToken: string | null = null;

  setToken(token: string | null) {
    this.accessToken = token;
  }

  getToken(): string | null {
    return this.accessToken;
  }

  private async request<T>(
    path: string,
    options: RequestInit = {},
  ): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string> || {}),
    };

    if (this.accessToken) {
      headers['Authorization'] = `Bearer ${this.accessToken}`;
    }

    const response = await fetch(`${API_BASE_URL}${path}`, {
      ...options,
      headers,
    });

    if (response.status === 401) {
      // Try to refresh the token
      const refreshed = await this.tryRefresh();
      if (refreshed) {
        headers['Authorization'] = `Bearer ${this.accessToken}`;
        const retryResponse = await fetch(`${API_BASE_URL}${path}`, {
          ...options,
          headers,
        });
        if (!retryResponse.ok) {
          throw new ApiError(retryResponse.status, await retryResponse.text());
        }
        if (retryResponse.status === 204) {
          return undefined as T;
        }
        const retryText = await retryResponse.text();
        if (!retryText) return undefined as T;
        return JSON.parse(retryText) as T;
      }
      // Refresh failed, redirect to the Modern UI's native login (TMAIL-327).
      // Using `window.location.hash` keeps us inside /modern/index.html
      // instead of bouncing out to the classic SPA's /login URL.
      window.location.hash = '#/login';
      throw new ApiError(401, 'Session expired');
    }

    if (!response.ok) {
      const body = await response.text();
      throw new ApiError(response.status, body);
    }

    // Fix: backend handlers like POST /api/drafts and POST /api/quota/sync
    // return 201/200 with an empty body. response.json() on an empty body
    // throws "JSON.parse: unexpected end of data". Read text first and parse
    // only when there's something to parse.
    if (response.status === 204) {
      return undefined as T;
    }
    const text = await response.text();
    if (!text) {
      return undefined as T;
    }
    return JSON.parse(text) as T;
  }

  private async tryRefresh(): Promise<boolean> {
    // TMAIL-327: "remember me = off" puts the token pair in sessionStorage,
    // so check both stores. Whichever store held the refresh token also
    // gets the rotated tokens written back so the session boundary survives.
    const fromLocal = localStorage.getItem('refresh_token');
    const fromSession = sessionStorage.getItem('refresh_token');
    const refreshToken = fromLocal || fromSession;
    if (!refreshToken) return false;
    const store: Storage = fromLocal ? localStorage : sessionStorage;

    try {
      const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
      });

      if (!response.ok) return false;

      const data = await response.json();
      this.accessToken = data.access_token;
      store.setItem('access_token', data.access_token);
      store.setItem('refresh_token', data.refresh_token);
      return true;
    } catch {
      return false;
    }
  }

  get<T>(path: string): Promise<T> {
    return this.request<T>(path);
  }

  post<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: 'POST',
      body: body ? JSON.stringify(body) : undefined,
    });
  }

  put<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: 'PUT',
      body: body ? JSON.stringify(body) : undefined,
    });
  }

  patch<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: 'PATCH',
      body: body ? JSON.stringify(body) : undefined,
    });
  }

  delete<T>(path: string, body?: unknown): Promise<T> {
    return this.request<T>(path, {
      method: 'DELETE',
      body: body ? JSON.stringify(body) : undefined,
    });
  }
}

export class ApiError extends Error {
  status: number;
  body: string;

  constructor(status: number, body: string) {
    super(`API Error ${status}: ${body}`);
    this.status = status;
    this.body = body;
  }
}

export const apiClient = new ApiClient();
