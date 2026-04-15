// Added: Higher-level responsive breakpoint hook for layout components (TMAIL-33)
import { useMediaQuery } from './useMediaQuery';

// NOTE: Breakpoints align with CSS media queries in App.css
const MOBILE_QUERY = '(max-width: 767px)';
const TABLET_QUERY = '(min-width: 768px) and (max-width: 1024px)';
const DESKTOP_QUERY = '(min-width: 1025px)';

export interface ResponsiveState {
  isMobile: boolean;
  isTablet: boolean;
  isDesktop: boolean;
}

/**
 * Hook that returns the current responsive breakpoint state.
 * - mobile: < 768px
 * - tablet: 768px - 1024px
 * - desktop: > 1024px
 */
export function useResponsive(): ResponsiveState {
  const isMobile = useMediaQuery(MOBILE_QUERY);
  const isTablet = useMediaQuery(TABLET_QUERY);
  const isDesktop = useMediaQuery(DESKTOP_QUERY);

  return { isMobile, isTablet, isDesktop };
}
