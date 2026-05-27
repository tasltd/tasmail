/**
 * PURPOSE: Wrap matched keyword substrings of plain text in <mark> elements
 * so search result rows can show what matched the query (TMAIL-32).
 * CONSTRAINTS: Case-insensitive, regex-safe (special chars escaped),
 * skips empty / whitespace-only keywords, never returns raw HTML strings.
 */
import type { ReactNode } from 'react';

// NOTE: Pull keywords from a free-text query by splitting on whitespace and
// dropping tokens shorter than 2 chars so single-letter noise doesn't flood
// the row with <mark>s.
export function tokenizeQuery(query: string | null | undefined): string[] {
  if (!query) return [];
  return query
    .split(/\s+/)
    .map((t) => t.trim())
    .filter((t) => t.length >= 2);
}

export function highlightKeywords(text: string | null | undefined, keywords: string[]): ReactNode {
  if (!text) return text ?? '';
  const valid = keywords.filter((k) => k && k.trim().length > 0);
  if (valid.length === 0) return text;

  const escaped = valid.map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const pattern = new RegExp(`(${escaped.join('|')})`, 'gi');
  const parts = text.split(pattern);

  return parts.map((part, i) => {
    const matched = valid.some((k) => k.toLowerCase() === part.toLowerCase());
    if (matched) {
      return (
        <mark key={i} className="search-highlight" data-testid="search-highlight">
          {part}
        </mark>
      );
    }
    return part;
  });
}
