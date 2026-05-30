// Added (TMAIL-327): Forgot-password screen at /#/forgot-password.
//
// NOTE: The backend does not yet expose a self-service password-reset
// endpoint (no migration, no handler — verified with `grep "password.*reset"
// backend/src`). Rather than ship a form that posts to nothing, this page
// gives the user actionable next steps: contact their admin (BYOK customers
// usually self-manage) or reach support. When the backend reset flow lands,
// swap this static notice for the real form.
import { Link } from 'react-router';
import { Mail, Shield } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { AuthLayout } from './AuthLayout';

export function ForgotPasswordPage() {
  return (
    <AuthLayout
      title="Reset your password"
      subtitle="We'll help you regain access to your account"
      footer={
        <>
          Remember it after all?{' '}
          <Link to="/login" className="text-primary font-medium hover:underline">
            Back to sign in
          </Link>
        </>
      }
    >
      <div className="space-y-5 text-sm">
        <p className="text-muted-foreground">
          Self-service password reset isn't available yet on TASMail. Here's how to
          get back in:
        </p>

        <div className="rounded-md border bg-muted/30 p-4 space-y-3">
          <div className="flex gap-3">
            <Shield className="size-5 text-primary shrink-0 mt-0.5" aria-hidden="true" />
            <div>
              <p className="font-medium">Workspace member?</p>
              <p className="text-muted-foreground">
                Ask your TASMail administrator to reset your password from the
                admin console.
              </p>
            </div>
          </div>
          <div className="flex gap-3">
            <Mail className="size-5 text-primary shrink-0 mt-0.5" aria-hidden="true" />
            <div>
              <p className="font-medium">Individual / BYOK account?</p>
              <p className="text-muted-foreground">
                Email{' '}
                <a
                  href="mailto:support@techatscale.io"
                  className="text-primary hover:underline"
                >
                  support@techatscale.io
                </a>{' '}
                from a verified address and we'll walk you through identity
                verification.
              </p>
            </div>
          </div>
        </div>

        <Button asChild className="w-full">
          <Link to="/login">Back to sign in</Link>
        </Button>
      </div>
    </AuthLayout>
  );
}
