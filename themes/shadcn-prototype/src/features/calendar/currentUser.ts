// TMAIL-351: read the authenticated user's email from the JWT in either
// localStorage (remember-me on) or sessionStorage (remember-me off). The
// existing src/lib/jwt.ts helper only checks localStorage, and rewriting
// it would risk other call sites; this calendar-scoped helper is the
// minimum change.

interface MinimalClaims {
  username?: string;
  sub?: string;
}

export function currentUserEmail(): string | null {
  if (typeof window === 'undefined') return null;
  const token =
    window.localStorage.getItem('access_token') ??
    window.sessionStorage.getItem('access_token');
  if (!token) return null;
  try {
    const payload = token.split('.')[1];
    const claims = JSON.parse(atob(payload)) as MinimalClaims;
    return claims.username ?? null;
  } catch {
    return null;
  }
}
