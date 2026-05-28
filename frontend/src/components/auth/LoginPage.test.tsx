import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { LoginPage } from './LoginPage';
// Added (TMAIL-273): Needed to construct a 423 ApiError for the lockout test.
import { ApiError } from '../../api/client';

describe('LoginPage', () => {
  const mockOnLogin = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders login form with email and password fields', () => {
    render(<LoginPage onLogin={mockOnLogin} />);

    expect(screen.getByLabelText('Email')).toBeInTheDocument();
    expect(screen.getByLabelText('Password')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Sign In' })).toBeInTheDocument();
  });

  it('renders TASMail branding', () => {
    render(<LoginPage onLogin={mockOnLogin} />);
    // Changed: TasmailLogo also renders the brand name as an aria-label /
    // <title>, so getByText('TASMail') now returns multiple nodes. Anchor on
    // the H1 explicitly and use the post-BYOK-pivot subtitle copy.
    expect(screen.getByRole('heading', { level: 1, name: 'TASMail' })).toBeInTheDocument();
    expect(screen.getByText('Webmail for any IMAP/SMTP server')).toBeInTheDocument();
  });

  it('shows error when submitting empty form', async () => {
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    expect(screen.getByText('Username and password are required')).toBeInTheDocument();
    expect(mockOnLogin).not.toHaveBeenCalled();
  });

  it('shows error when only username is provided', async () => {
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'user@test.com' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    expect(screen.getByText('Username and password are required')).toBeInTheDocument();
  });

  it('calls onLogin with username and password', async () => {
    mockOnLogin.mockResolvedValue(undefined);
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'user@test.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'password123' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(mockOnLogin).toHaveBeenCalledWith('user@test.com', 'password123');
    });
  });

  it('shows "Signing in..." while loading', async () => {
    // Make onLogin hang
    mockOnLogin.mockImplementation(() => new Promise(() => {}));
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'user@test.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'pass' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(screen.getByText('Signing in...')).toBeInTheDocument();
    });
  });

  it('shows error message on login failure', async () => {
    mockOnLogin.mockRejectedValue(new Error('Invalid credentials'));
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'user@test.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(screen.getByText('Invalid credentials')).toBeInTheDocument();
    });
  });

  it('disables button while loading', async () => {
    mockOnLogin.mockImplementation(() => new Promise(() => {}));
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'a@b.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'p' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Signing in...' })).toBeDisabled();
    });
  });

  it('has autocomplete attributes for credentials', () => {
    render(<LoginPage onLogin={mockOnLogin} />);

    expect(screen.getByLabelText('Email')).toHaveAttribute('autocomplete', 'username');
    expect(screen.getByLabelText('Password')).toHaveAttribute('autocomplete', 'current-password');
  });

  // Added (TMAIL-273): When the backend returns HTTP 423 from /api/auth/login
  // the page must render the generic "Account temporarily locked..." message
  // regardless of what JSON the backend put in the body. No countdown, no
  // remaining-attempts hint — the whole point is non-enumeration.
  it('shows generic lockout message on HTTP 423 ApiError', async () => {
    mockOnLogin.mockRejectedValue(
      new ApiError(423, JSON.stringify({ error: 'whatever-the-backend-said' })),
    );
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'locked@test.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      expect(
        screen.getByText('Account temporarily locked. Try again later.'),
      ).toBeInTheDocument();
    });
    // The backend body text must NOT leak into the UI.
    expect(screen.queryByText(/whatever-the-backend-said/)).not.toBeInTheDocument();
  });

  // Added (TMAIL-273): Non-423 ApiErrors keep the existing error.message
  // pass-through path so 401 "Invalid credentials" still surfaces normally.
  it('passes through non-423 ApiError messages', async () => {
    mockOnLogin.mockRejectedValue(new ApiError(401, 'Invalid credentials body'));
    render(<LoginPage onLogin={mockOnLogin} />);

    fireEvent.change(screen.getByLabelText('Email'), { target: { value: 'user@test.com' } });
    fireEvent.change(screen.getByLabelText('Password'), { target: { value: 'nope' } });
    fireEvent.click(screen.getByRole('button', { name: 'Sign In' }));

    await waitFor(() => {
      // ApiError.message is "API Error 401: Invalid credentials body"
      expect(screen.getByText(/API Error 401/)).toBeInTheDocument();
    });
  });
});
