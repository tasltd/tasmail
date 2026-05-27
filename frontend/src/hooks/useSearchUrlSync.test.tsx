import { describe, it, expect, beforeEach } from 'vitest';
import { render, act } from '@testing-library/react';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { useSearchUrlSync, paramsFromUrl, urlFromParams } from './useSearchUrlSync';
import { useMailStore } from '../stores/mailStore';

function ProbeAndSync() {
  useSearchUrlSync();
  const loc = useLocation();
  return <div data-testid="loc">{loc.search}</div>;
}

function getStore() {
  return useMailStore.getState();
}

describe('paramsFromUrl', () => {
  it('returns empty state when URL has no search params', () => {
    const r = paramsFromUrl(new URLSearchParams(''));
    expect(r.query).toBe('');
    expect(r.advanced).toBeNull();
  });

  it('hydrates plain query', () => {
    const r = paramsFromUrl(new URLSearchParams('?q=hello'));
    expect(r.query).toBe('hello');
    expect(r.advanced).toBeNull();
  });

  it('hydrates advanced filters with the original query', () => {
    const sp = new URLSearchParams(
      '?q=budget&from=alice@example.com&hasAttachment=1&dateFrom=2026-01-01',
    );
    const r = paramsFromUrl(sp);
    expect(r.query).toBe('budget');
    expect(r.advanced).toEqual({
      query: 'budget',
      from: 'alice@example.com',
      hasAttachment: true,
      dateFrom: '2026-01-01',
    });
  });
});

describe('urlFromParams', () => {
  it('omits everything for an empty search', () => {
    expect(urlFromParams('', null).toString()).toBe('');
  });

  it('writes only non-empty fields', () => {
    const sp = urlFromParams('hello', {
      query: 'hello',
      from: 'bob@example.com',
      isUnread: true,
      isStarred: false,
    });
    expect(sp.get('q')).toBe('hello');
    expect(sp.get('from')).toBe('bob@example.com');
    expect(sp.get('isUnread')).toBe('1');
    expect(sp.has('isStarred')).toBe(false);
  });
});

describe('useSearchUrlSync', () => {
  beforeEach(() => {
    // Reset the zustand store between tests.
    useMailStore.setState({
      searchQuery: '',
      advancedSearch: null,
      viewMode: 'list',
      selectedFolder: 'INBOX',
      selectedUid: null,
    });
  });

  it('hydrates the store from ?q on first render', () => {
    render(
      <MemoryRouter initialEntries={['/app?q=hello']}>
        <ProbeAndSync />
      </MemoryRouter>,
    );
    expect(getStore().searchQuery).toBe('hello');
    expect(getStore().viewMode).toBe('search');
  });

  it('hydrates advanced search filters from URL', () => {
    render(
      <MemoryRouter initialEntries={['/app?q=budget&from=alice@example.com&hasAttachment=1']}>
        <ProbeAndSync />
      </MemoryRouter>,
    );
    const s = getStore();
    expect(s.searchQuery).toBe('budget');
    expect(s.advancedSearch).toEqual({
      query: 'budget',
      from: 'alice@example.com',
      hasAttachment: true,
    });
  });

  it('writes the search query to the URL when the store changes', () => {
    const { getByTestId } = render(
      <MemoryRouter initialEntries={['/app']}>
        <ProbeAndSync />
      </MemoryRouter>,
    );
    act(() => {
      useMailStore.getState().setSearchQuery('payroll');
    });
    expect(getByTestId('loc').textContent).toBe('?q=payroll');
  });

  it('writes advanced filter fields to the URL', () => {
    const { getByTestId } = render(
      <MemoryRouter initialEntries={['/app']}>
        <ProbeAndSync />
      </MemoryRouter>,
    );
    act(() => {
      useMailStore.getState().setAdvancedSearch({
        query: 'q1',
        from: 'a@b.com',
        dateFrom: '2026-01-01',
        isUnread: true,
      });
    });
    const search = getByTestId('loc').textContent ?? '';
    expect(search).toContain('q=q1');
    expect(search).toContain('from=a%40b.com');
    expect(search).toContain('dateFrom=2026-01-01');
    expect(search).toContain('isUnread=1');
  });

  it('clears all search params when the store is cleared', () => {
    const { getByTestId } = render(
      <MemoryRouter initialEntries={['/app?q=foo&from=x@y.com']}>
        <ProbeAndSync />
      </MemoryRouter>,
    );
    // Sanity: hydrated.
    expect(getStore().searchQuery).toBe('foo');
    act(() => {
      useMailStore.getState().setAdvancedSearch(null);
      useMailStore.getState().setSearchQuery('');
    });
    expect(getByTestId('loc').textContent).toBe('');
  });
});
