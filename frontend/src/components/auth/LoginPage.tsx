import React from 'react';
import { useState, useEffect } from 'react';
import { Mail } from 'lucide-react';
import { listLoginProviders, getAuthorizeUrl } from '../../api/oidc';
import type { OidcLoginProvider } from '../../api/oidc';

interface LoginPageProps {
  onLogin: (username: string, password: string) => Promise<void>;
}

export function LoginPage({ onLogin }: LoginPageProps) {
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  // Added: OIDC providers for social login buttons (TMAIL-99)
  const [oidcProviders, setOidcProviders] = useState<OidcLoginProvider[]>([]);
  const [oidcLoading, setOidcLoading] = useState<string | null>(null);

  // Added: Fetch active OIDC providers on mount for social login display
  useEffect(() => {
    listLoginProviders()
      .then(setOidcProviders)
      .catch(() => {
        // NOTE: Silently ignore — social login buttons just won't appear
      });
  }, []);

  // Added: Handle OIDC provider login — redirect to authorization URL
  const handleOidcLogin = async (providerId: string) => {
    setOidcLoading(providerId);
    setError('');
    try {
      const { authorize_url, state } = await getAuthorizeUrl(providerId);
      // Added: Store state token for CSRF validation on callback
      sessionStorage.setItem('oidc_state', state);
      window.location.href = authorize_url;
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start OIDC login');
      setOidcLoading(null);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username || !password) {
      setError('Username and password are required');
      return;
    }

    setLoading(true);
    setError('');

    try {
      await onLogin(username, password);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-page">
      <div className="login-card">
        <div className="login-card__header">
          <Mail size={40} />
          <h1>TASMail</h1>
          <p>Self-hosted email service</p>
        </div>

        {error && <div className="login-card__error">{error}</div>}

        <form onSubmit={handleSubmit} className="login-card__form">
          <div className="form-group">
            <label htmlFor="username">Email</label>
            <input
              id="username"
              type="text"
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="user@example.com"
              autoComplete="username"
              autoFocus
            />
          </div>

          <div className="form-group">
            <label htmlFor="password">Password</label>
            <input
              id="password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="Password"
              autoComplete="current-password"
            />
          </div>

          <button
            type="submit"
            className="btn btn--primary btn--full"
            disabled={loading}
          >
            {loading ? 'Signing in...' : 'Sign In'}
          </button>
        </form>

        {/* Added: OIDC social login buttons below the form (TMAIL-99) */}
        {oidcProviders.length > 0 && (
          <div className="login-card__oidc">
            <div className="login-card__divider">
              <span>or</span>
            </div>
            {oidcProviders.map((provider) => (
              <button
                key={provider.id}
                className="btn btn--secondary btn--full"
                style={{ marginTop: '8px', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '8px' }}
                onClick={() => handleOidcLogin(provider.id)}
                disabled={oidcLoading === provider.id}
              >
                {provider.icon_url && (
                  <img
                    src={provider.icon_url}
                    alt=""
                    style={{ width: '20px', height: '20px' }}
                  />
                )}
                {oidcLoading === provider.id
                  ? 'Redirecting...'
                  : provider.button_label || `Sign in with ${provider.name}`}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
