// Added (TMAIL-327): Native Modern UI signup screen at /#/signup.
// Calls POST /api/auth/signup, which returns a JWT pair and creates the
// account. After successful signup we bounce the user to the classic
// /onboarding wizard — the BYOK IMAP/SMTP onboarding flow has not been
// ported to the Modern UI yet, so the classic flow is still the correct
// place to attach a mail server.
import { useState } from 'react';
import type * as React from 'react';
import { Link } from 'react-router';
import { Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { signup } from '@/api/auth';
import { ApiError } from '@/api/client';
import { AuthLayout } from './AuthLayout';

export function SignupPage() {
  const [email, setEmail] = useState('');
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError('');

    if (!email || !password) {
      setError('Email and password are required.');
      return;
    }
    if (password.length < 8) {
      setError('Password must be at least 8 characters.');
      return;
    }
    if (password !== confirm) {
      setError('Passwords do not match.');
      return;
    }

    setLoading(true);
    try {
      await signup({
        email: email.trim().toLowerCase(),
        password,
        display_name: displayName.trim() || undefined,
      });
      // BYOK onboarding (attach IMAP/SMTP server) only lives in the classic
      // SPA today — full-page navigation drops the new user there.
      window.location.href = '/onboarding';
    } catch (err) {
      if (err instanceof ApiError && err.status === 409) {
        setError('An account with that email already exists.');
      } else {
        setError(err instanceof Error ? err.message : 'Sign-up failed.');
      }
    } finally {
      setLoading(false);
    }
  }

  return (
    <AuthLayout
      title="Create your TASMail account"
      subtitle="Bring your own IMAP/SMTP server"
      footer={
        <>
          Already have an account?{' '}
          <Link to="/login" className="text-primary font-medium hover:underline">
            Sign in
          </Link>
        </>
      }
    >
      {error && (
        <Alert variant="destructive" className="mb-4" role="alert">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}

      <form onSubmit={handleSubmit} className="space-y-4" aria-label="Create account">
        <div className="space-y-2">
          <Label htmlFor="email">Email</Label>
          <Input
            id="email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="you@example.com"
            autoComplete="email"
            autoFocus
            required
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="display_name" className="flex items-center gap-1">
            Display name
            <span className="text-xs font-normal text-muted-foreground">(optional)</span>
          </Label>
          <Input
            id="display_name"
            type="text"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            placeholder="Your name"
            autoComplete="name"
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="password">Password</Label>
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="At least 8 characters"
            autoComplete="new-password"
            minLength={8}
            required
          />
        </div>

        <div className="space-y-2">
          <Label htmlFor="confirm">Confirm password</Label>
          <Input
            id="confirm"
            type="password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            autoComplete="new-password"
            minLength={8}
            required
          />
        </div>

        <Button type="submit" className="w-full" disabled={loading}>
          {loading && <Loader2 className="size-4 animate-spin" aria-hidden="true" />}
          {loading ? 'Creating account…' : 'Create account'}
        </Button>
      </form>

      <div className="mt-6 pt-4 border-t text-center text-xs text-muted-foreground">
        <a href="/signup" className="hover:underline" data-testid="use-classic-signup">
          Use classic signup
        </a>
      </div>
    </AuthLayout>
  );
}
