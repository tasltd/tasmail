// Added (TMAIL-327): Native Modern UI login screen. Renders inside
// /modern/index.html#/login so the Modern UI no longer has to bounce out to
// the classic SPA for authentication.
//
// Behaviour:
//   - Calls POST /api/auth/login via the modern apiClient (same JWT storage
//     keys as classic, so a session started here is interchangeable).
//   - Surfaces OIDC providers from GET /api/auth/oidc/providers.
//   - Honors a `remember_me` toggle: when off, we move the tokens out of
//     localStorage into sessionStorage after a successful sign-in so the
//     session ends with the browser session.
//   - Maps HTTP 423 Locked to a generic message (TMAIL-273 parity).
//   - Surfaces a "Use classic login" link as the explicit fallback the
//     ticket requires.
import { useEffect, useState } from 'react';
import type * as React from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Checkbox } from '@/components/ui/checkbox';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { login } from '@/api/auth';
import { listLoginProviders, getAuthorizeUrl, type OidcLoginProvider } from '@/api/oidc';
import { ApiError } from '@/api/client';
import { AuthLayout } from './AuthLayout';

const LOCKED_MESSAGE = 'Account temporarily locked. Try again later.';

export function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const nextPath = searchParams.get('next') || '/';

  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [rememberMe, setRememberMe] = useState(true);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const [oidcProviders, setOidcProviders] = useState<OidcLoginProvider[]>([]);
  const [oidcLoading, setOidcLoading] = useState<string | null>(null);

  useEffect(() => {
    listLoginProviders()
      .then(setOidcProviders)
      .catch(() => {
        // NOTE: Silently swallow — no providers configured is fine.
      });
  }, []);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!username || !password) {
      setError('Email and password are required');
      return;
    }
    setLoading(true);
    setError('');
    try {
      await login({ username: username.trim(), password });
      // When the user opts out of "remember me" we copy the tokens that
      // login() wrote into localStorage over to sessionStorage and remove
      // the persistent copies, so closing the browser ends the session.
      if (!rememberMe) {
        const access = localStorage.getItem('access_token');
        const refresh = localStorage.getItem('refresh_token');
        if (access) sessionStorage.setItem('access_token', access);
        if (refresh) sessionStorage.setItem('refresh_token', refresh);
        localStorage.removeItem('access_token');
        localStorage.removeItem('refresh_token');
      }
      navigate(nextPath, { replace: true });
    } catch (err) {
      if (err instanceof ApiError && err.status === 423) {
        setError(LOCKED_MESSAGE);
      } else if (err instanceof ApiError && err.status === 401) {
        setError('Incorrect email or password.');
      } else {
        setError(err instanceof Error ? err.message : 'Login failed');
      }
    } finally {
      setLoading(false);
    }
  }

  async function handleOidcLogin(providerId: string) {
    setOidcLoading(providerId);
    setError('');
    try {
      const { authorize_url, state } = await getAuthorizeUrl(providerId);
      sessionStorage.setItem('oidc_state', state);
      window.location.href = authorize_url;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start OIDC login');
      setOidcLoading(null);
    }
  }

  return (
    <AuthLayout
      title="Sign in to TASMail"
      subtitle="Webmail for any IMAP/SMTP server"
      footer={
        <>
          New to TASMail?{' '}
          <Link to="/signup" className="text-primary font-medium hover:underline">
            Create an account
          </Link>
        </>
      }
    >
      {error && (
        <Alert variant="destructive" className="mb-4" role="alert">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <form onSubmit={handleSubmit} className="space-y-4" aria-label="Sign in">
        <div className="space-y-2">
          <Label htmlFor="username">Email</Label>
          <Input
            id="username"
            type="email"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            placeholder="you@example.com"
            autoComplete="username"
            autoFocus
            required
          />
        </div>

        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <Label htmlFor="password">Password</Label>
            <Link
              to="/forgot-password"
              className="text-xs text-primary hover:underline"
            >
              Forgot password?
            </Link>
          </div>
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
            required
          />
        </div>

        <div className="flex items-center gap-2">
          <Checkbox
            id="remember_me"
            checked={rememberMe}
            onCheckedChange={(checked) => setRememberMe(checked === true)}
          />
          <Label htmlFor="remember_me" className="font-normal cursor-pointer">
            Remember me on this device
          </Label>
        </div>

        <Button type="submit" className="w-full" disabled={loading}>
          {loading && <Loader2 className="size-4 animate-spin" aria-hidden="true" />}
          {loading ? 'Signing in…' : 'Sign in'}
        </Button>
      </form>

      {oidcProviders.length > 0 && (
        <>
          <div className="relative my-6">
            <div className="absolute inset-0 flex items-center">
              <span className="w-full border-t" />
            </div>
            <div className="relative flex justify-center text-xs">
              <span className="bg-card px-2 text-muted-foreground">or continue with</span>
            </div>
          </div>
          <div className="space-y-2">
            {oidcProviders.map((provider) => (
              <Button
                key={provider.id}
                type="button"
                variant="outline"
                className="w-full"
                onClick={() => handleOidcLogin(provider.id)}
                disabled={oidcLoading === provider.id}
              >
                {provider.icon_url && (
                  <img src={provider.icon_url} alt="" className="size-4" />
                )}
                {oidcLoading === provider.id
                  ? 'Redirecting…'
                  : provider.button_label || `Sign in with ${provider.name}`}
              </Button>
            ))}
          </div>
        </>
      )}

      <div className="mt-6 pt-4 border-t text-center text-xs text-muted-foreground">
        <a href="/login" className="hover:underline" data-testid="use-classic-login">
          Use classic login
        </a>
      </div>
    </AuthLayout>
  );
}
