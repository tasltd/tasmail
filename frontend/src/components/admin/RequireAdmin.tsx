// TMAIL-197: route guard that gates admin pages on the JWT's is_admin claim.
//
// Sits inside RequireAuth (which only checks "is there a token?") and adds
// the role check. Decodes the JWT locally — the backend re-verifies on every
// request, so this is purely a UX gate and won't grant access to non-admins
// even if the token is forged.
import { Navigate } from 'react-router-dom';

interface JwtClaims {
  sub: string;
  username?: string;
  is_admin?: boolean;
  exp: number;
  iat: number;
}

function decodeClaims(token: string): JwtClaims | null {
  try {
    const payload = token.split('.')[1];
    return JSON.parse(atob(payload)) as JwtClaims;
  } catch {
    return null;
  }
}

export function RequireAdmin({ children }: { children: React.ReactElement }) {
  const token = localStorage.getItem('access_token');
  if (!token) return <Navigate to="/login" replace />;
  const claims = decodeClaims(token);
  if (!claims?.is_admin) {
    // Non-admin trying to load /admin/* — bounce them back to /app, not /login,
    // because they ARE authenticated; they just don't have the role.
    return (
      <div style={{ padding: '48px 24px', maxWidth: 640, margin: '0 auto', textAlign: 'center' }}>
        <h1 style={{ fontSize: 28, marginBottom: 8 }}>Admin only</h1>
        <p style={{ color: 'var(--color-text-secondary, #64748b)', marginBottom: 24 }}>
          This area is restricted to operators with the <code>is_admin</code> flag set on their mailbox.
          You're signed in but don't have the admin role.
        </p>
        <a href="/app" className="btn btn--primary">Back to mailbox</a>
      </div>
    );
  }
  return children;
}
