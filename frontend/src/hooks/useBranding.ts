// Added: TMAIL-111 — applies branding from /api/branding to the running app.
// Fetches once via TanStack Query (same cache key as BrandingManager so admin
// edits invalidate this), then writes CSS custom properties onto :root, updates
// document.title and the favicon link, and injects custom_css into a managed
// <style> tag. Components consume the brand via the `--brand-*` variables.
import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { getBranding } from '../api/branding';
import type { Branding } from '../api/branding';

const CUSTOM_CSS_STYLE_ID = 'tasmail-branding-custom-css';
const FAVICON_LINK_ID = 'tasmail-branding-favicon';

// NOTE: keep these in sync with the keys consumed by CSS. They MUST match the
// `--brand-*` namespace so the existing `--accent` etc. variables in index.css
// stay untouched and routes that don't apply branding still render correctly.
const CSS_VAR_PRIMARY = '--brand-primary-color';
const CSS_VAR_SECONDARY = '--brand-secondary-color';
const CSS_VAR_ACCENT = '--brand-accent-color';
const CSS_VAR_LOGIN_BG = '--brand-login-background';

function applyCssVariables(branding: Branding): void {
  const root = document.documentElement;
  root.style.setProperty(CSS_VAR_PRIMARY, branding.primary_color);
  root.style.setProperty(CSS_VAR_SECONDARY, branding.secondary_color);
  root.style.setProperty(CSS_VAR_ACCENT, branding.accent_color);
  if (branding.login_background_url) {
    root.style.setProperty(CSS_VAR_LOGIN_BG, `url("${branding.login_background_url}")`);
  } else {
    root.style.removeProperty(CSS_VAR_LOGIN_BG);
  }
}

function applyDocumentTitle(branding: Branding): void {
  // NOTE: keeps the marketing tagline so the public landing page title stays
  // descriptive — admins changing app_name only swap the brand prefix.
  document.title = `${branding.app_name} — webmail for any IMAP/SMTP server`;
}

function applyFavicon(branding: Branding): void {
  if (!branding.favicon_url) return;
  let link = document.getElementById(FAVICON_LINK_ID) as HTMLLinkElement | null;
  if (!link) {
    link = document.createElement('link');
    link.id = FAVICON_LINK_ID;
    link.rel = 'icon';
    document.head.appendChild(link);
  }
  link.href = branding.favicon_url;
}

function applyCustomCss(branding: Branding): void {
  const css = branding.custom_css ?? '';
  let styleEl = document.getElementById(CUSTOM_CSS_STYLE_ID) as HTMLStyleElement | null;
  if (!css) {
    if (styleEl) styleEl.textContent = '';
    return;
  }
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = CUSTOM_CSS_STYLE_ID;
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = css;
}

export function applyBranding(branding: Branding): void {
  applyCssVariables(branding);
  applyDocumentTitle(branding);
  applyFavicon(branding);
  applyCustomCss(branding);
}

/**
 * PURPOSE: Fetch branding from /api/branding and apply it to the document.
 * EXTERNAL: GET /api/branding (public endpoint, no auth required).
 * Re-runs whenever the query invalidates (admin save/reset triggers this).
 */
export function useBranding() {
  const query = useQuery<Branding>({
    queryKey: ['branding'],
    queryFn: getBranding,
    // NOTE: branding rarely changes, so keep it stale-stable for an hour.
    // BrandingManager invalidates this on save/reset, so admin edits still
    // propagate immediately.
    staleTime: 60 * 60 * 1000,
  });

  useEffect(() => {
    if (query.data) applyBranding(query.data);
  }, [query.data]);

  return query;
}
