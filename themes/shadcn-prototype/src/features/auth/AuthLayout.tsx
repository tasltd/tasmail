// Added (TMAIL-327): Shared auth screen layout — centered card with the
// TASMail brand header. Used by LoginPage, SignupPage, and
// ForgotPasswordPage so the public surfaces look consistent.
import type { ReactNode } from 'react';
import { Mail } from 'lucide-react';

interface AuthLayoutProps {
  title: string;
  subtitle?: string;
  children: ReactNode;
  footer?: ReactNode;
}

export function AuthLayout({ title, subtitle, children, footer }: AuthLayoutProps) {
  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-zinc-50 to-zinc-100 dark:from-zinc-950 dark:to-zinc-900 px-4 py-8">
      <div className="w-full max-w-md">
        <div className="flex flex-col items-center gap-2 mb-6">
          <div
            className="flex items-center justify-center size-14 rounded-xl bg-primary text-primary-foreground"
            aria-hidden="true"
          >
            <Mail className="size-7" />
          </div>
          <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
          {subtitle && (
            <p className="text-sm text-muted-foreground text-center">{subtitle}</p>
          )}
        </div>

        <div className="rounded-xl border bg-card text-card-foreground shadow-sm p-6">
          {children}
        </div>

        {footer && (
          <div className="mt-6 text-center text-sm text-muted-foreground">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
