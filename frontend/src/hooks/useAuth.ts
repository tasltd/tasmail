import { useState, useEffect, useCallback } from 'react';
import { login as apiLogin, logout as apiLogout, restoreSession } from '../api/auth';
import type { LoginRequest } from '../types/auth';

export function useAuth() {
  const [isAuthenticated, setIsAuthenticated] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const restored = restoreSession();
    setIsAuthenticated(restored);
    setIsLoading(false);
  }, []);

  const login = useCallback(async (credentials: LoginRequest) => {
    await apiLogin(credentials);
    setIsAuthenticated(true);
  }, []);

  const logout = useCallback(async () => {
    await apiLogout();
    setIsAuthenticated(false);
  }, []);

  return { isAuthenticated, isLoading, login, logout };
}
