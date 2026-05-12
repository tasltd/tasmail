// Added: Public landing page for TASMail at the root path (/).
// Marketing-style hero, feature grid, pricing, footer. CTA buttons route to /login.
import { Link } from 'react-router-dom';
import './LandingPage.css';

export function LandingPage() {
  return (
    <div className="landing">
      <header className="landing-header">
        <div className="landing-header__inner">
          <Link to="/" className="landing-header__brand" aria-label="TASMail home">
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect width="20" height="16" x="2" y="4" rx="2" />
              <path d="m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" />
            </svg>
            <span>TASMail</span>
          </Link>
          <nav className="landing-header__nav">
            <a href="#features">Features</a>
            <a href="#pricing">Pricing</a>
            <a href="#deploy">Self-host</a>
            <Link to="/login" className="landing-btn landing-btn--ghost">Sign in</Link>
            <Link to="/signup" className="landing-btn landing-btn--primary">Get started</Link>
          </nav>
        </div>
      </header>

      <section className="landing-hero">
        <div className="landing-hero__inner">
          <span className="landing-hero__badge">Bring your own IMAP &middot; works with Gmail, Outlook, Zoho, FastMail…</span>
          <h1 className="landing-hero__title">
            One <span className="landing-hero__accent">webmail UI</span> for all your accounts.
          </h1>
          <p className="landing-hero__subtitle">
            TASMail is a fast, modern webmail client that connects to <em>any</em> IMAP/SMTP server &mdash; the one you already use. Sign up, plug in your credentials, and read your real mailbox in a clean interface that works the same on every device. We never store your email; only the encrypted credentials needed to fetch it.
          </p>
          <div className="landing-hero__ctas">
            <Link to="/signup" className="landing-btn landing-btn--primary landing-btn--lg">
              Create your account
            </Link>
            <Link to="/login" className="landing-btn landing-btn--ghost landing-btn--lg">
              Sign in
            </Link>
          </div>
          <p className="landing-hero__caption">
            Powers <code>mail.techatscale.io</code> &middot; runs on a single $10/month VPS &middot; unlimited users
          </p>
        </div>
      </section>

      <section id="features" className="landing-features">
        <div className="landing-features__inner">
          <h2 className="landing-section__title">Everything you need from email — nothing you don&apos;t</h2>
          <div className="landing-features__grid">
            <FeatureCard icon="message-square" title="Modern webmail">
              React 19 SPA with rich-text composer (TipTap), real-time WebSocket push, search, snooze, undo-send, and keyboard shortcuts.
            </FeatureCard>
            <FeatureCard icon="shield" title="Privacy by design">
              Your IMAP server, your storage. No third-party tracking, no ad targeting, no data mining. Encrypted at rest and in transit.
            </FeatureCard>
            <FeatureCard icon="cpu" title="Lightweight">
              Rust backend uses under 100&nbsp;MB of RAM and serves API responses in well under 200&nbsp;ms.
            </FeatureCard>
            <FeatureCard icon="users" title="Team-ready">
              Shared mailboxes, distribution groups, delegation, retention policies, e-discovery, and SAML/OIDC SSO out of the box.
            </FeatureCard>
            <FeatureCard icon="smartphone" title="Mobile-first">
              Native Flutter apps for Android and iOS, plus a PWA with offline cache and background sync.
            </FeatureCard>
            <FeatureCard icon="lock" title="Battle-tested core">
              Built on Postfix (SMTP) and Dovecot (IMAP) — the same software that runs the majority of the Internet&apos;s mail.
            </FeatureCard>
          </div>
        </div>
      </section>

      <section id="pricing" className="landing-pricing">
        <div className="landing-pricing__inner">
          <h2 className="landing-section__title">Simple pricing</h2>
          <p className="landing-pricing__subtitle">
            Self-hosted is always free. Hosted plans are coming soon and will be billed in Ghana cedis (GHS) via Paystack, Mastercard, Cybersource, or bank transfer — the same providers PayPro uses.
          </p>
          <div className="landing-pricing__grid">
            <PricingCard
              name="Self-host"
              price="Free"
              tagline="Run TASMail on your own VPS"
              features={[
                'Unlimited mailboxes',
                'Full source code (MIT)',
                'Bring your own DNS, certs, and storage',
                'Community support',
              ]}
              cta="View on GitHub"
              ctaHref="https://github.com/tasltd/tasmail"
              ctaExternal
            />
            <PricingCard
              name="Hosted"
              price="GHS 25"
              priceSuffix="/user/month"
              tagline="Managed by Tech at Scale"
              features={[
                'Mailboxes on @techatscale.io or your own domain',
                '25 GB per user',
                'Daily backups + 30-day retention',
                'Email + chat support',
              ]}
              cta="Sign in"
              ctaHref="/login"
              highlight
            />
            <PricingCard
              name="Enterprise"
              price="Custom"
              tagline="For organisations"
              features={[
                'Dedicated infrastructure',
                'SAML / OIDC SSO',
                'SLA + onboarding',
                'Compliance reporting (eDiscovery, DLP, retention)',
              ]}
              cta="Contact sales"
              ctaHref="mailto:hello@techatscale.io"
              ctaExternal
            />
          </div>
        </div>
      </section>

      <section id="deploy" className="landing-deploy">
        <div className="landing-deploy__inner">
          <h2 className="landing-section__title">Deploy in minutes</h2>
          <p className="landing-deploy__subtitle">
            Clone the repository, point a $10 VPS at your domain, run the setup script. That&apos;s the entire deployment.
          </p>
          <pre className="landing-deploy__code">{`git clone https://github.com/tasltd/tasmail
cd tasmail/deploy/scripts
sudo ./setup-all.sh --domain example.com --hostname mail.example.com`}</pre>
        </div>
      </section>

      <footer className="landing-footer">
        <div className="landing-footer__inner">
          <div className="landing-footer__col">
            <strong>TASMail</strong>
            <p>Self-hosted email by Tech at Scale.</p>
          </div>
          <div className="landing-footer__col">
            <strong>Product</strong>
            <a href="#features">Features</a>
            <a href="#pricing">Pricing</a>
            <Link to="/login">Sign in</Link>
          </div>
          <div className="landing-footer__col">
            <strong>Company</strong>
            <a href="https://techatscale.io">Tech at Scale</a>
            <a href="mailto:hello@techatscale.io">hello@techatscale.io</a>
          </div>
          <div className="landing-footer__col">
            <strong>Source</strong>
            <a href="https://github.com/tasltd/tasmail">GitHub</a>
          </div>
        </div>
        <div className="landing-footer__copy">
          &copy; {new Date().getFullYear()} Tech at Scale Ltd. MIT licensed.
        </div>
      </footer>
    </div>
  );
}

interface FeatureProps {
  icon: 'message-square' | 'shield' | 'cpu' | 'users' | 'smartphone' | 'lock';
  title: string;
  children: React.ReactNode;
}

function FeatureCard({ icon, title, children }: FeatureProps) {
  return (
    <div className="landing-feature">
      <div className="landing-feature__icon" aria-hidden>
        <FeatureIcon name={icon} />
      </div>
      <h3 className="landing-feature__title">{title}</h3>
      <p className="landing-feature__body">{children}</p>
    </div>
  );
}

function FeatureIcon({ name }: { name: FeatureProps['icon'] }) {
  // Inline SVG icons (lucide-style) — keeps bundle tiny without adding lucide-react.
  const common = { width: 22, height: 22, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const };
  switch (name) {
    case 'message-square': return <svg {...common}><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>;
    case 'shield': return <svg {...common}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10"/></svg>;
    case 'cpu': return <svg {...common}><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><path d="M9 1v3m6-3v3M9 20v3m6-3v3M20 9h3m-3 6h3M1 9h3m-3 6h3"/></svg>;
    case 'users': return <svg {...common}><path d="M16 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="8.5" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/></svg>;
    case 'smartphone': return <svg {...common}><rect x="5" y="2" width="14" height="20" rx="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>;
    case 'lock': return <svg {...common}><rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>;
  }
}

interface PricingProps {
  name: string;
  price: string;
  priceSuffix?: string;
  tagline: string;
  features: string[];
  cta: string;
  ctaHref: string;
  ctaExternal?: boolean;
  highlight?: boolean;
}

function PricingCard({ name, price, priceSuffix, tagline, features, cta, ctaHref, ctaExternal, highlight }: PricingProps) {
  return (
    <div className={`landing-price-card${highlight ? ' landing-price-card--highlight' : ''}`}>
      <div className="landing-price-card__name">{name}</div>
      <div className="landing-price-card__price">
        {price}
        {priceSuffix && <span className="landing-price-card__suffix">{priceSuffix}</span>}
      </div>
      <div className="landing-price-card__tagline">{tagline}</div>
      <ul className="landing-price-card__features">
        {features.map((f) => <li key={f}>{f}</li>)}
      </ul>
      {ctaExternal ? (
        <a className="landing-btn landing-btn--primary landing-btn--block" href={ctaHref} target="_blank" rel="noreferrer">{cta}</a>
      ) : (
        <Link className="landing-btn landing-btn--primary landing-btn--block" to={ctaHref}>{cta}</Link>
      )}
    </div>
  );
}
