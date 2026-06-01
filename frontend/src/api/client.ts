import { API_BASE_URL } from '../utils/constants';

// Fix (TMAIL-413): Endpoints where a 401 means "wrong credentials" rather than
// "session expired". The client must NOT redirect to /login for these paths —
// the calling component (LoginPage / SignupPage) handles the ApiError itself
// so it can render an inline `.login-card__error` message.
const AUTH_NO_REDIRECT_PATHS = ['/auth/login', '/auth/signup', '/auth/refresh'];

function isAuthEndpoint(path: string): boolean {
  return AUTH_NO_REDIRECT_PATHS.some((p) => path === p || path.startsWith(`${p}?`));
}

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

    // Fix (TMAIL-413): A 401 from /auth/login, /auth/signup, or /auth/refresh
    // means "wrong credentials" — NOT "session expired". Treating those like
    // session-expired triggers a refresh-then-redirect to /login, which wipes
    // the LoginPage's error state before React can render `.login-card__error`.
    // The caller (LoginPage) handles invalid-credential ApiErrors itself.
    if (response.status === 401 && !isAuthEndpoint(path)) {
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
        if (retryResponse.status === 204) return undefined as T;
        const retryText = await retryResponse.text();
        if (!retryText) return undefined as T;
        return JSON.parse(retryText) as T;
      }
      // Refresh failed, redirect to login
      window.location.href = '/login';
      throw new ApiError(401, 'Session expired');
    }

    if (!response.ok) {
      const body = await response.text();
      throw new ApiError(response.status, body);
    }

    // Fix: handlers that return 201/200 with empty body (POST /api/drafts,
    // POST /api/quota/sync, etc.) trip response.json() with
    // "JSON.parse: unexpected end of data". Read text first and parse only
    // when non-empty.
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
    const refreshToken = localStorage.getItem('refresh_token');
    if (!refreshToken) return false;

    try {
      const response = await fetch(`${API_BASE_URL}/auth/refresh`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ refresh_token: refreshToken }),
      });

      if (!response.ok) return false;

      const data = await response.json();
      this.accessToken = data.access_token;
      localStorage.setItem('access_token', data.access_token);
      localStorage.setItem('refresh_token', data.refresh_token);
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
