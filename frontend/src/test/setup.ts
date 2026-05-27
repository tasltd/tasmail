import '@testing-library/jest-dom/vitest';

// Added: ResizeObserver polyfill for libraries (e.g. FullCalendar) that use it in jsdom (TMAIL-118)
if (typeof globalThis.ResizeObserver === 'undefined') {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}
