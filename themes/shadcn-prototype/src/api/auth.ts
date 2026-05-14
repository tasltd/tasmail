import type { LoginRequest, TokenPair } from '../types/auth';
import { apiClient } from './client';

export async function login(credentials: LoginRequest): Promise<TokenPair> {
  const tokens = await apiClient.post<TokenPair>('/auth/login', credentials);
  apiClient.setToken(tokens.access_token);
  localStorage.setItem('access_token', tokens.access_token);
  localStorage.setItem('refresh_token', tokens.refresh_token);
  return tokens;
}

// Added: BYOK signup. The backend returns a token pair so the user is logged in
// immediately and can be routed straight to the IMAP/SMTP onboarding wizard.
export interface SignupRequest {
  email: string;
  password: string;
  display_name?: string;
}

export async function signup(req: SignupRequest): Promise<TokenPair> {
  const tokens = await apiClient.post<TokenPair>('/auth/signup', req);
  apiClient.setToken(tokens.access_token);
  localStorage.setItem('access_token', tokens.access_token);
  localStorage.setItem('refresh_token', tokens.refresh_token);
  return tokens;
}

export async function logout(): Promise<void> {
  try {
    await apiClient.post('/auth/logout');
  } finally {
    apiClient.setToken(null);
    localStorage.removeItem('access_token');
    localStorage.removeItem('refresh_token');
  }
}

export function restoreSession(): boolean {
  const token = localStorage.getItem('access_token');
  if (token) {
    apiClient.setToken(token);
    return true;
  }
  return false;
}
