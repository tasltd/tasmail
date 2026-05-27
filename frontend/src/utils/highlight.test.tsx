import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { highlightKeywords, tokenizeQuery } from './highlight';

describe('tokenizeQuery', () => {
  it('returns empty array for empty / nullish input', () => {
    expect(tokenizeQuery('')).toEqual([]);
    expect(tokenizeQuery(null)).toEqual([]);
    expect(tokenizeQuery(undefined)).toEqual([]);
  });

  it('splits on whitespace and drops tokens shorter than 2 chars', () => {
    expect(tokenizeQuery('hello world a b')).toEqual(['hello', 'world']);
  });

  it('collapses runs of whitespace', () => {
    expect(tokenizeQuery('  foo   bar   ')).toEqual(['foo', 'bar']);
  });
});

describe('highlightKeywords', () => {
  it('returns text unchanged when no keywords match', () => {
    const { container } = render(<>{highlightKeywords('Hello world', ['python'])}</>);
    expect(container.querySelector('mark')).toBeNull();
    expect(container.textContent).toBe('Hello world');
  });

  it('wraps matched keyword in <mark> with the search-highlight class', () => {
    const { container } = render(<>{highlightKeywords('Quarterly budget review', ['budget'])}</>);
    const marks = container.querySelectorAll('mark.search-highlight');
    expect(marks).toHaveLength(1);
    expect(marks[0].textContent).toBe('budget');
  });

  it('matches case-insensitively but preserves original casing', () => {
    const { container } = render(<>{highlightKeywords('Budget vs BUDGET', ['budget'])}</>);
    const marks = container.querySelectorAll('mark');
    expect(marks).toHaveLength(2);
    expect(marks[0].textContent).toBe('Budget');
    expect(marks[1].textContent).toBe('BUDGET');
  });

  it('handles multiple keywords in the same string', () => {
    const { container } = render(
      <>{highlightKeywords('alice and bob met', ['alice', 'bob'])}</>,
    );
    const marks = Array.from(container.querySelectorAll('mark')).map((m) => m.textContent);
    expect(marks).toEqual(['alice', 'bob']);
  });

  it('escapes regex metacharacters in keywords', () => {
    const { container } = render(
      <>{highlightKeywords('price is $5.99 today', ['$5.99'])}</>,
    );
    const marks = container.querySelectorAll('mark');
    expect(marks).toHaveLength(1);
    expect(marks[0].textContent).toBe('$5.99');
  });

  it('drops empty / whitespace-only keywords without throwing', () => {
    const { container } = render(<>{highlightKeywords('hello world', ['', '   '])}</>);
    expect(container.querySelector('mark')).toBeNull();
    expect(container.textContent).toBe('hello world');
  });

  it('returns empty string for nullish text', () => {
    expect(highlightKeywords(null, ['x'])).toBe('');
    expect(highlightKeywords(undefined, ['x'])).toBe('');
  });
});
