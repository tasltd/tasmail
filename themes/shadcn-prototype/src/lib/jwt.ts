// TMAIL-232: tiny helper that mirrors frontend/src/components/admin/RequireAdmin.tsx.
// Pure UX gate — backend re-verifies on every request, so a forged token still
// can't reach admin data.
export interface JwtClaims {
  sub: string;
  username?: string;
  is_admin?: boolean;
  exp: number;
  iat: number;
}

export function decodeAccessClaims(): JwtClaims | null {
  const token = localStorage.getItem('access_token');
  if (!token) return null;
  try {
    const payload = token.split('.')[1];
    return JSON.parse(atob(payload)) as JwtClaims;
  } catch {
    return null;
  }
}

export function isAdmin(): boolean {
  return Boolean(decodeAccessClaims()?.is_admin);
}
