import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useUiStore } from './uiStore';

// Mock localStorage
const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: vi.fn((key: string) => store[key] ?? null),
    setItem: vi.fn((key: string, value: string) => { store[key] = value; }),
    clear: () => { store = {}; },
  };
})();
Object.defineProperty(globalThis, 'localStorage', { value: localStorageMock });

describe('uiStore', () => {
  beforeEach(() => {
    localStorageMock.clear();
    useUiStore.setState({ sidebarOpen: true, theme: 'light' });
  });

  it('defaults to sidebar open and light theme', () => {
    const state = useUiStore.getState();
    expect(state.sidebarOpen).toBe(true);
    expect(state.theme).toBe('light');
  });

  it('toggleSidebar flips sidebarOpen', () => {
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarOpen).toBe(false);
    useUiStore.getState().toggleSidebar();
    expect(useUiStore.getState().sidebarOpen).toBe(true);
  });

  it('setSidebarOpen sets explicit value', () => {
    useUiStore.getState().setSidebarOpen(false);
    expect(useUiStore.getState().sidebarOpen).toBe(false);
    useUiStore.getState().setSidebarOpen(true);
    expect(useUiStore.getState().sidebarOpen).toBe(true);
  });

  it('toggleTheme switches between light and dark', () => {
    useUiStore.getState().toggleTheme();
    expect(useUiStore.getState().theme).toBe('dark');
    useUiStore.getState().toggleTheme();
    expect(useUiStore.getState().theme).toBe('light');
  });

  it('toggleTheme persists to localStorage', () => {
    useUiStore.getState().toggleTheme();
    expect(localStorageMock.setItem).toHaveBeenCalledWith('theme', 'dark');
  });
});
