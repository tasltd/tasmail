// Added: TMAIL-111 — verifies useBranding applies branding to the document
// (CSS variables, title, favicon link, custom_css style tag). Uses applyBranding
// directly to avoid the React Query + component-render dance and keep the
// assertion focused on the side effects that matter.
import { describe, it, expect, beforeEach } from 'vitest';
import { applyBranding } from './useBranding';
import type { Branding } from '../api/branding';

function makeBranding(overrides: Partial<Branding> = {}): Branding {
  return {
    id: 'b-1',
    app_name: 'TASMail',
    logo_url: null,
    favicon_url: null,
    primary_color: '#2563eb',
    secondary_color: '#1e40af',
    accent_color: '#3b82f6',
    login_background_url: null,
    custom_css: null,
    footer_text: null,
    support_email: null,
    support_url: null,
    updated_at: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

function resetDocument() {
  const root = document.documentElement;
  root.style.removeProperty('--brand-primary-color');
  root.style.removeProperty('--brand-secondary-color');
  root.style.removeProperty('--brand-accent-color');
  root.style.removeProperty('--brand-login-background');
  document.title = '';
  document.querySelectorAll('#tasmail-branding-custom-css, #tasmail-branding-favicon').forEach((el) => el.remove());
}

describe('applyBranding', () => {
  beforeEach(() => {
    resetDocument();
  });

  it('writes brand CSS custom properties onto :root', () => {
    applyBranding(makeBranding({ primary_color: '#ff0000', secondary_color: '#00ff00', accent_color: '#0000ff' }));
    const root = document.documentElement;
    expect(root.style.getPropertyValue('--brand-primary-color')).toBe('#ff0000');
    expect(root.style.getPropertyValue('--brand-secondary-color')).toBe('#00ff00');
    expect(root.style.getPropertyValue('--brand-accent-color')).toBe('#0000ff');
  });

  it('sets document.title from app_name', () => {
    applyBranding(makeBranding({ app_name: 'AcmeMail' }));
    expect(document.title).toContain('AcmeMail');
  });

  it('writes a managed <link> with the favicon_url when provided', () => {
    applyBranding(makeBranding({ favicon_url: 'https://cdn.example/acme.ico' }));
    const link = document.getElementById('tasmail-branding-favicon') as HTMLLinkElement | null;
    expect(link).not.toBeNull();
    expect(link!.rel).toBe('icon');
    expect(link!.href).toBe('https://cdn.example/acme.ico');
  });

  it('reuses the same <link> on subsequent applies and updates its href', () => {
    applyBranding(makeBranding({ favicon_url: 'https://cdn.example/v1.ico' }));
    applyBranding(makeBranding({ favicon_url: 'https://cdn.example/v2.ico' }));
    const links = document.querySelectorAll('#tasmail-branding-favicon');
    expect(links.length).toBe(1);
    expect((links[0] as HTMLLinkElement).href).toBe('https://cdn.example/v2.ico');
  });

  it('does not create a favicon link when favicon_url is null', () => {
    applyBranding(makeBranding({ favicon_url: null }));
    expect(document.getElementById('tasmail-branding-favicon')).toBeNull();
  });

  it('injects custom_css into a managed <style> tag', () => {
    applyBranding(makeBranding({ custom_css: 'body { background: pink; }' }));
    const styleEl = document.getElementById('tasmail-branding-custom-css') as HTMLStyleElement | null;
    expect(styleEl).not.toBeNull();
    expect(styleEl!.tagName).toBe('STYLE');
    expect(styleEl!.textContent).toBe('body { background: pink; }');
  });

  it('clears the custom_css style tag content when custom_css is null', () => {
    applyBranding(makeBranding({ custom_css: 'body { color: red; }' }));
    applyBranding(makeBranding({ custom_css: null }));
    const styleEl = document.getElementById('tasmail-branding-custom-css') as HTMLStyleElement | null;
    // NOTE: the element is kept so we don't churn the DOM, but its content is cleared.
    if (styleEl) expect(styleEl.textContent).toBe('');
  });

  it('sets a login-background CSS var when login_background_url is provided', () => {
    applyBranding(makeBranding({ login_background_url: 'https://cdn.example/bg.jpg' }));
    const value = document.documentElement.style.getPropertyValue('--brand-login-background');
    expect(value).toBe('url("https://cdn.example/bg.jpg")');
  });

  it('removes the login-background CSS var when login_background_url is null', () => {
    applyBranding(makeBranding({ login_background_url: 'https://cdn.example/bg.jpg' }));
    applyBranding(makeBranding({ login_background_url: null }));
    const value = document.documentElement.style.getPropertyValue('--brand-login-background');
    expect(value).toBe('');
  });
});
