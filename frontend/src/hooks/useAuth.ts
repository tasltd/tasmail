import { useState, useEffect, useCallback } from 'react';
import { login as apiLogin, logout as apiLogout, restoreSession } from '../api/auth';
import type { LoginRequest } from '../types/auth';

// Added (TMAIL-398): mirrors the JwtClaims decoded by RequireAdmin so the
// sidebar can gate the Admin entry from non-admin users. The backend still
// re-verifies the role on every request — this is purely a UX gate.
interface JwtClaims {
  sub: string;
  username?: string;
  is_admin?: boolean;
  exp: number;
  iat: number;
}

function decodeIsAdmin(): boolean {
  const token = typeof localStorage !== 'undefined' ? localStorage.getItem('access_token') : null;
  if (!token) return false;
  try {
    const payload = token.split('.')[1];
    if (!payload) return false;
    const claims = JSON.parse(atob(payload)) as JwtClaims;
    return Boolean(claims.is_admin);
  } catch {
    return false;
  }
}

export function useAuth() {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  // Added (TMAIL-398): decoded is_admin claim — Sidebar uses this to hide
  // the Admin entry from non-admin users. Re-evaluated on every auth flip.
  const [isAdmin, setIsAdmin] = useState(false);

  useEffect(() => {
    const restored = restoreSession();
    setIsAuthenticated(restored);
    setIsAdmin(restored ? decodeIsAdmin() : false);
    setIsLoading(false);
  }, []);

  const login = useCallback(async (credentials: LoginRequest) => {
    await apiLogin(credentials);
    setIsAuthenticated(true);
    setIsAdmin(decodeIsAdmin());
  }, []);

  const logout = useCallback(async () => {
    await apiLogout();
    setIsAuthenticated(false);
    setIsAdmin(false);
  }, []);

  return { isAuthenticated, isLoading, isAdmin, login, logout };
}
