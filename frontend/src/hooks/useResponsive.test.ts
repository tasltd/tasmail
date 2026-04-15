import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useResponsive } from './useResponsive';

describe('useResponsive', () => {
  let matchState: Map<string, boolean>;

  beforeEach(() => {
    matchState = new Map();

    // Added: Mock matchMedia to simulate different viewport sizes
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: matchState.get(query) ?? false,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('detects mobile viewport (< 768px)', () => {
    matchState.set('(max-width: 767px)', true);
    matchState.set('(min-width: 768px) and (max-width: 1024px)', false);
    matchState.set('(min-width: 1025px)', false);

    const { result } = renderHook(() => useResponsive());
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isTablet).toBe(false);
    expect(result.current.isDesktop).toBe(false);
  });

  it('detects tablet viewport (768px - 1024px)', () => {
    matchState.set('(max-width: 767px)', false);
    matchState.set('(min-width: 768px) and (max-width: 1024px)', true);
    matchState.set('(min-width: 1025px)', false);

    const { result } = renderHook(() => useResponsive());
    expect(result.current.isMobile).toBe(false);
    expect(result.current.isTablet).toBe(true);
    expect(result.current.isDesktop).toBe(false);
  });

  it('detects desktop viewport (> 1024px)', () => {
    matchState.set('(max-width: 767px)', false);
    matchState.set('(min-width: 768px) and (max-width: 1024px)', false);
    matchState.set('(min-width: 1025px)', true);

    const { result } = renderHook(() => useResponsive());
    expect(result.current.isMobile).toBe(false);
    expect(result.current.isTablet).toBe(false);
    expect(result.current.isDesktop).toBe(true);
  });

  it('returns all false when no breakpoint matches (edge case)', () => {
    // NOTE: This should not happen in practice but tests defensive behavior
    matchState.set('(max-width: 767px)', false);
    matchState.set('(min-width: 768px) and (max-width: 1024px)', false);
    matchState.set('(min-width: 1025px)', false);

    const { result } = renderHook(() => useResponsive());
    expect(result.current.isMobile).toBe(false);
    expect(result.current.isTablet).toBe(false);
    expect(result.current.isDesktop).toBe(false);
  });
});
