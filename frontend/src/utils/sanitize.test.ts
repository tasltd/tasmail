import { describe, it, expect } from 'vitest';
import { sanitizeHtml } from './sanitize';

describe('sanitizeHtml', () => {
  it('allows basic HTML tags', () => {
    const input = '<p>Hello <strong>world</strong></p>';
    const result = sanitizeHtml(input);
    expect(result).toContain('<p>');
    expect(result).toContain('<strong>');
    expect(result).toContain('Hello');
  });

  it('strips script tags', () => {
    const input = '<p>Hello</p><script>alert("xss")</script>';
    const result = sanitizeHtml(input);
    expect(result).not.toContain('<script>');
    expect(result).not.toContain('alert');
    expect(result).toContain('Hello');
  });

  it('strips event handlers', () => {
    const input = '<img src="x" onerror="alert(1)">';
    const result = sanitizeHtml(input);
    expect(result).not.toContain('onerror');
  });

  it('allows href on links', () => {
    const input = '<a href="https://example.com">Link</a>';
    const result = sanitizeHtml(input);
    expect(result).toContain('href="https://example.com"');
  });

  it('adds target="_blank" to links', () => {
    const input = '<a href="https://example.com">Link</a>';
    const result = sanitizeHtml(input);
    expect(result).toContain('target="_blank"');
    expect(result).toContain('rel="noopener noreferrer"');
  });

  it('strips javascript: protocol from links', () => {
    const input = '<a href="javascript:alert(1)">XSS</a>';
    const result = sanitizeHtml(input);
    expect(result).not.toContain('javascript:');
  });

  it('allows img tags with src', () => {
    const input = '<img src="https://example.com/img.png" alt="photo">';
    const result = sanitizeHtml(input);
    expect(result).toContain('src="https://example.com/img.png"');
    expect(result).toContain('alt="photo"');
  });

  it('allows table tags for email layouts', () => {
    const input = '<table><tr><td>Cell</td></tr></table>';
    const result = sanitizeHtml(input);
    expect(result).toContain('<table>');
    expect(result).toContain('<td>');
  });

  it('strips data attributes', () => {
    const input = '<div data-custom="malicious">Content</div>';
    const result = sanitizeHtml(input);
    expect(result).not.toContain('data-custom');
    expect(result).toContain('Content');
  });

  it('allows style attribute', () => {
    const input = '<p style="color: red;">Styled</p>';
    const result = sanitizeHtml(input);
    expect(result).toContain('style=');
  });

  it('handles empty string', () => {
    expect(sanitizeHtml('')).toBe('');
  });
});
