// TMAIL-174: standalone /pricing page with detail + interactive cost calculator + FAQ.
//
// Composes the same header/footer as the LandingPage. The calculator slider lets
// visitors estimate their monthly bill before signing up, with the locale-aware
// USD line we use elsewhere.
import { useState } from 'react';
import { Link } from 'react-router-dom';
import { TasmailLogo } from '../shared/TasmailLogo';
import './LandingPage.css';
import './PricingPage.css';

const GHS_PER_GB = 1.00;
const MONTHLY_MIN = 5.00;
const GHS_TO_USD = 0.067;
const GH_LOCALE_RE = /(^en-GH$|^en-GH-|-GH$|-gh$|^ak\b|^tw\b|^ee\b|^ga\b|-Gh-)/i;

function isGhanaLocale(): boolean {
  if (typeof navigator === 'undefined') return true;
  const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
  return langs.some((l) => GH_LOCALE_RE.test(l));
}

function compute(gb: number): { ghs: number; minimumApplied: boolean } {
  const billed = Math.ceil(gb);
  const raw = billed * GHS_PER_GB;
  if (raw < MONTHLY_MIN) return { ghs: MONTHLY_MIN, minimumApplied: true };
  return { ghs: raw, minimumApplied: false };
}

export function PricingPage() {
  const [gb, setGb] = useState(20);
  const showUsd = !isGhanaLocale();
  const { ghs, minimumApplied } = compute(gb);

  return (
    <div className="landing">
      <header className="landing-header">
        <div className="landing-header__inner">
          <Link to="/" className="landing-header__brand">
            <TasmailLogo size={32} />
            <span>TASMail</span>
          </Link>
          <nav className="landing-header__nav">
            <Link to="/#features">Features</Link>
            <Link to="/pricing">Pricing</Link>
            <Link to="/#enterprise-quote">Enterprise</Link>
            <Link to="/login" className="landing-btn landing-btn--ghost">Sign in</Link>
            <Link to="/signup" className="landing-btn landing-btn--primary">Get started</Link>
          </nav>
        </div>
      </header>

      <section className="pp-hero">
        <h1>Pricing</h1>
        <p>
          One simple per-GB rate for everyone on the BYOK plan, custom quotes for everything bigger.
          We settle in Ghana cedis (GHS) — visitors outside Ghana see an indicative USD line next to every price.
        </p>
      </section>

      <section className="pp-calc">
        <h2>Estimate your bill</h2>
        <p>Drag the slider to see how much your TASMail-attached mailbox would cost per month.</p>
        <div className="pp-calc__panel">
          <input
            type="range"
            min={0}
            max={500}
            value={gb}
            onChange={(e) => setGb(parseInt(e.target.value, 10))}
            className="pp-calc__slider"
          />
          <div className="pp-calc__values">
            <span><strong>{gb}</strong> GB stored</span>
            <span className="pp-calc__price">
              GHS {ghs.toFixed(2)}{showUsd && <span className="pp-calc__usd"> (≈ ${(ghs * GHS_TO_USD).toFixed(2)} USD)</span>}
              <span className="pp-calc__per"> / month</span>
            </span>
          </div>
          {minimumApplied && (
            <p className="pp-calc__hint">The GHS {MONTHLY_MIN.toFixed(2)} monthly minimum applies — it kicks in below ~{Math.ceil(MONTHLY_MIN / GHS_PER_GB)} GB.</p>
          )}
        </div>
        <p className="pp-calc__cta">
          <Link className="landing-btn landing-btn--primary landing-btn--lg" to="/signup">Sign up — start at GHS {MONTHLY_MIN.toFixed(2)}</Link>
        </p>
      </section>

      <section className="pp-tiers">
        <h2>Two ways to use TASMail</h2>
        <div className="pp-tier-grid">
          <div className="pp-tier pp-tier--primary">
            <header>
              <span className="pp-tier__badge">Recommended</span>
              <h3>BYOK</h3>
              <p>Bring your own IMAP/SMTP server. Connect Gmail, Outlook, Zoho, FastMail, your corporate Exchange, or any standard mail server — TASMail proxies the mailbox you already use.</p>
            </header>
            <ul>
              <li>GHS {GHS_PER_GB.toFixed(2)} / GB-month</li>
              <li>GHS {MONTHLY_MIN.toFixed(2)} monthly minimum</li>
              <li>No mailbox provided — encrypted credentials only</li>
              <li>Unlimited devices · iOS, Android, web, PWA</li>
              <li>Email + chat support</li>
            </ul>
            <Link className="landing-btn landing-btn--primary landing-btn--block" to="/signup">Start with BYOK</Link>
          </div>

          <div className="pp-tier">
            <header>
              <span className="pp-tier__badge pp-tier__badge--enterprise">Enterprise</span>
              <h3>Custom deployment</h3>
              <p>Single-tenant TASMail on your cloud or ours, sized to your team and your compliance posture.</p>
            </header>
            <ul>
              <li>Negotiated annual pricing</li>
              <li>Dedicated infrastructure or on-premise install</li>
              <li>SAML / OIDC SSO · SCIM provisioning</li>
              <li>White-glove onboarding + SLA</li>
              <li>Compliance reporting (eDiscovery, DLP, retention, audit)</li>
            </ul>
            <Link className="landing-btn landing-btn--ghost landing-btn--block" to="/#enterprise-quote">Request a quote</Link>
          </div>
        </div>
      </section>

      <section className="pp-providers">
        <h2>Settled via the same providers PayPro uses</h2>
        <p>Every invoice is denominated in GHS and charged through one of four providers in your account preferences:</p>
        <ul className="pp-providers__list">
          <li>Paystack (cards + Mobile Money)</li>
          <li>Mastercard MPGS</li>
          <li>Cybersource invoicing</li>
          <li>Bank Transfer (manual)</li>
        </ul>
      </section>

      <section className="pp-faq">
        <h2>Frequently asked</h2>
        <details>
          <summary>What counts as "storage"?</summary>
          <p>The <code>used_bytes</code> reported by your IMAP server. We snapshot it nightly and bill the monthly average — so a one-day spike doesn't move your bill, and a 50% deletion mid-month is reflected proportionally.</p>
        </details>
        <details>
          <summary>Are attachments included?</summary>
          <p>Yes. Attachments are part of <code>used_bytes</code> on the IMAP server, so they're already in your storage figure.</p>
        </details>
        <details>
          <summary>Why GHS and not USD?</summary>
          <p>Our payment providers settle in cedis. The USD numbers we show are indicative only — your statement will be in GHS and converted by your bank at their checkout rate.</p>
        </details>
        <details>
          <summary>What if my mailbox is empty?</summary>
          <p>The GHS {MONTHLY_MIN.toFixed(2)} monthly minimum keeps the lights on. It applies to anyone using less than ~{Math.ceil(MONTHLY_MIN / GHS_PER_GB)} GB.</p>
        </details>
        <details>
          <summary>Can I cancel any time?</summary>
          <p>Yes. Cancel from /billing — your subscription stops at the end of the current billing period and you keep access until then.</p>
        </details>
      </section>

      <footer className="landing-footer">
        <div className="landing-footer__inner">
          <div className="landing-footer__col">
            <strong>TASMail</strong>
            <p>Webmail for any IMAP/SMTP server, by Tech at Scale.</p>
          </div>
          <div className="landing-footer__col">
            <strong>Product</strong>
            <Link to="/#features">Features</Link>
            <Link to="/pricing">Pricing</Link>
            <Link to="/login">Sign in</Link>
          </div>
          <div className="landing-footer__col">
            <strong>Company</strong>
            <a href="https://techatscale.io">Tech at Scale</a>
            <a href="mailto:hello@techatscale.io">hello@techatscale.io</a>
          </div>
        </div>
        <div className="landing-footer__copy">
          &copy; {new Date().getFullYear()} Tech at Scale Ltd. MIT licensed.
        </div>
      </footer>
    </div>
  );
}
