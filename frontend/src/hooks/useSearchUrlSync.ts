/**
 * PURPOSE: Two-way sync between the mail store's search state and the URL
 * query string (TMAIL-32) so:
 *   - users can bookmark / share a search,
 *   - browser back/forward restores the search,
 *   - the search survives a page refresh.
 *
 * CONSTRAINTS:
 *   - Read-on-mount uses a ref to run hydration exactly once per route.
 *   - Subsequent writes are guarded against echoing the URL update back
 *     into the store (no infinite loop).
 *   - Empty / no-op state writes a clean URL with no search params.
 */
import { useEffect, useRef } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useMailStore } from '../stores/mailStore';
import type { AdvancedSearchParams } from '../api/messages';

// Exported for unit testing of the (de)serializer logic.
export function paramsFromUrl(sp: URLSearchParams): {
  query: string;
  advanced: AdvancedSearchParams | null;
} {
  const q = sp.get('q') ?? '';
  const from = sp.get('from') ?? '';
  const to = sp.get('to') ?? '';
  const subject = sp.get('subject') ?? '';
  const dateFrom = sp.get('dateFrom') ?? '';
  const dateTo = sp.get('dateTo') ?? '';
  const hasAttachment = sp.get('hasAttachment') === '1';
  const isUnread = sp.get('isUnread') === '1';
  const isStarred = sp.get('isStarred') === '1';

  const hasAnyAdvanced =
    !!from || !!to || !!subject || !!dateFrom || !!dateTo ||
    hasAttachment || isUnread || isStarred;

  if (!hasAnyAdvanced) {
    return { query: q, advanced: null };
  }

  const advanced: AdvancedSearchParams = { query: q };
  if (from) advanced.from = from;
  if (to) advanced.to = to;
  if (subject) advanced.subject = subject;
  if (dateFrom) advanced.dateFrom = dateFrom;
  if (dateTo) advanced.dateTo = dateTo;
  if (hasAttachment) advanced.hasAttachment = true;
  if (isUnread) advanced.isUnread = true;
  if (isStarred) advanced.isStarred = true;
  return { query: q, advanced };
}

export function urlFromParams(
  query: string,
  advanced: AdvancedSearchParams | null,
): URLSearchParams {
  const next = new URLSearchParams();
  // NOTE: searchQuery and advanced.query may be set independently in the store
  // (AdvancedSearch.tsx copies one into the other on submit, but unit tests
  // and direct setAdvancedSearch callers may set only one). Prefer the
  // top-level value, fall back to the one nested in advanced.
  const effectiveQuery = query || advanced?.query || '';
  if (effectiveQuery) next.set('q', effectiveQuery);
  if (advanced) {
    if (advanced.from) next.set('from', advanced.from);
    if (advanced.to) next.set('to', advanced.to);
    if (advanced.subject) next.set('subject', advanced.subject);
    if (advanced.dateFrom) next.set('dateFrom', advanced.dateFrom);
    if (advanced.dateTo) next.set('dateTo', advanced.dateTo);
    if (advanced.hasAttachment) next.set('hasAttachment', '1');
    if (advanced.isUnread) next.set('isUnread', '1');
    if (advanced.isStarred) next.set('isStarred', '1');
  }
  return next;
}

// NOTE: Build a stable signature so a no-op rerender doesn't rewrite the URL.
function signature(sp: URLSearchParams): string {
  const keys = Array.from(sp.keys()).sort();
  return keys.map((k) => `${k}=${sp.get(k)}`).join('&');
}

export function useSearchUrlSync() {
  const [urlParams, setUrlParams] = useSearchParams();
  const searchQuery = useMailStore((s) => s.searchQuery);
  const advancedSearch = useMailStore((s) => s.advancedSearch);
  const setSearchQuery = useMailStore((s) => s.setSearchQuery);
  const setAdvancedSearch = useMailStore((s) => s.setAdvancedSearch);

  const hydrated = useRef(false);
  const lastWritten = useRef<string>('');

  // Hydrate once from the URL on first render.
  useEffect(() => {
    if (hydrated.current) return;
    hydrated.current = true;
    const { query, advanced } = paramsFromUrl(urlParams);
    if (advanced) {
      setAdvancedSearch(advanced);
      if (query) setSearchQuery(query);
    } else if (query) {
      setSearchQuery(query);
    }
    lastWritten.current = signature(urlParams);
  }, [urlParams, setSearchQuery, setAdvancedSearch]);

  // Push store -> URL whenever search state changes.
  useEffect(() => {
    if (!hydrated.current) return;
    const next = urlFromParams(searchQuery, advancedSearch);
    const sig = signature(next);
    if (sig === lastWritten.current) return;
    lastWritten.current = sig;
    // NOTE: 'replace' so a typing user doesn't flood browser history with one entry per keystroke.
    setUrlParams(next, { replace: true });
  }, [searchQuery, advancedSearch, setUrlParams]);
}
