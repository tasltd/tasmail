// TMAIL-184: enterprise quote-request form embedded on the landing page.
// Inline (not a modal) so it's reachable via the #enterprise-quote anchor and
// crawlable by search engines without JS gymnastics.
import { useState } from 'react';
import './EnterpriseQuoteForm.css';

interface QuoteResponse {
  id: string;
  status: string;
}

export function EnterpriseQuoteForm() {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [company, setCompany] = useState('');
  const [users, setUsers] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [success, setSuccess] = useState<{ id: string } | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError('');

    if (!name.trim() || !email.trim() || !message.trim()) {
      setError('Name, email, and message are required.');
      return;
    }

    setBusy(true);
    try {
      const resp = await fetch('/api/enterprise/quote-request', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          contact_name: name.trim(),
          contact_email: email.trim().toLowerCase(),
          company: company.trim() || undefined,
          estimated_users: users ? parseInt(users, 10) : undefined,
          message: message.trim(),
        }),
      });
      if (!resp.ok) {
        const body = await resp.json().catch(() => ({ error: `HTTP ${resp.status}` }));
        throw new Error(body.error ?? `HTTP ${resp.status}`);
      }
      const data = (await resp.json()) as QuoteResponse;
      setSuccess({ id: data.id });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Submission failed.');
    } finally {
      setBusy(false);
    }
  }

  if (success) {
    return (
      <div className="eqf-success" role="status">
        <div className="eqf-success__check">✓</div>
        <h3>Thanks — we&apos;ll be in touch within one business day.</h3>
        <p>Tracking id: <code>{success.id.slice(0, 8)}</code></p>
        <p className="eqf-success__hint">If you don&apos;t hear back, email us directly at <a href="mailto:hello@techatscale.io">hello@techatscale.io</a>.</p>
      </div>
    );
  }

  return (
    <form className="eqf" onSubmit={handleSubmit} noValidate>
      {error && <div className="eqf__error" role="alert">{error}</div>}

      <div className="eqf__row">
        <label className="eqf__field">
          <span>Name</span>
          <input
            id="eqf-name"
            type="text"
            autoComplete="name"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            maxLength={200}
          />
        </label>
        <label className="eqf__field">
          <span>Work email</span>
          <input
            id="eqf-email"
            type="email"
            autoComplete="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
          />
        </label>
      </div>

      <div className="eqf__row">
        <label className="eqf__field">
          <span>Company <span className="eqf__optional">(optional)</span></span>
          <input
            id="eqf-company"
            type="text"
            autoComplete="organization"
            value={company}
            onChange={(e) => setCompany(e.target.value)}
            maxLength={200}
          />
        </label>
        <label className="eqf__field">
          <span>Estimated users <span className="eqf__optional">(optional)</span></span>
          <input
            id="eqf-users"
            type="number"
            inputMode="numeric"
            min={1}
            max={1000000}
            value={users}
            onChange={(e) => setUsers(e.target.value)}
          />
        </label>
      </div>

      <label className="eqf__field">
        <span>What are you looking for?</span>
        <textarea
          id="eqf-message"
          rows={5}
          required
          maxLength={4000}
          placeholder="Tell us about your team size, hosting preferences (your cloud or ours), compliance requirements, and any timelines you're working to."
          value={message}
          onChange={(e) => setMessage(e.target.value)}
        />
      </label>

      <button type="submit" className="landing-btn landing-btn--primary landing-btn--lg" disabled={busy}>
        {busy ? 'Sending…' : 'Request a quote'}
      </button>

      <p className="eqf__hint">
        We respond within one business day. Your details only go to TASMail sales — never shared.
      </p>
    </form>
  );
}
