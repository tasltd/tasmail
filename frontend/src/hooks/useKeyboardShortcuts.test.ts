import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

// Added: Mock the mail store with controllable state
const mockSetViewMode = vi.fn();
const mockSetSelectedFolder = vi.fn();
const mockSetSelectedUid = vi.fn();
const mockSetSearchQuery = vi.fn();

let mockState = {
  selectedFolder: 'INBOX',
  selectedUid: null as number | null,
  viewMode: 'list' as string,
  searchQuery: '',
  setViewMode: mockSetViewMode,
  setSelectedFolder: mockSetSelectedFolder,
  setSelectedUid: mockSetSelectedUid,
  setSearchQuery: mockSetSearchQuery,
};

vi.mock('../stores/mailStore', () => ({
  useMailStore: Object.assign(
    (selector?: (s: typeof mockState) => unknown) => selector ? selector(mockState) : mockState,
    { getState: () => mockState },
  ),
}));

// Added: Mock API functions to prevent real HTTP calls
vi.mock('../api/messages', () => ({
  deleteMessage: vi.fn().mockResolvedValue(undefined),
  moveMessage: vi.fn().mockResolvedValue(undefined),
  flagMessage: vi.fn().mockResolvedValue(undefined),
}));

import { useKeyboardShortcuts } from './useKeyboardShortcuts';
import { deleteMessage, moveMessage, flagMessage } from '../api/messages';

/**
 * PURPOSE: Unit tests for Gmail-like keyboard shortcuts hook
 * CONSTRAINTS: Tests fire KeyboardEvent on document and assert store actions
 */
describe('useKeyboardShortcuts', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockState = {
      selectedFolder: 'INBOX',
      selectedUid: null,
      viewMode: 'list',
      searchQuery: '',
      setViewMode: mockSetViewMode,
      setSelectedFolder: mockSetSelectedFolder,
      setSelectedUid: mockSetSelectedUid,
      setSearchQuery: mockSetSearchQuery,
    };
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function fireKey(key: string, options: Partial<KeyboardEventInit> = {}) {
    const event = new KeyboardEvent('keydown', { key, bubbles: true, ...options });
    document.dispatchEvent(event);
  }

  it('skips shortcuts when user is typing in an input element', () => {
    renderHook(() => useKeyboardShortcuts());

    // NOTE: Create a focused input and dispatch the event on it
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    const event = new KeyboardEvent('keydown', { key: 'c', bubbles: true });
    Object.defineProperty(event, 'target', { value: input });
    document.dispatchEvent(event);

    expect(mockSetViewMode).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });

  it('skips shortcuts when user is typing in a textarea', () => {
    renderHook(() => useKeyboardShortcuts());

    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);
    textarea.focus();

    const event = new KeyboardEvent('keydown', { key: 'c', bubbles: true });
    Object.defineProperty(event, 'target', { value: textarea });
    document.dispatchEvent(event);

    expect(mockSetViewMode).not.toHaveBeenCalled();
    document.body.removeChild(textarea);
  });

  it('triggers compose mode when "c" is pressed', () => {
    renderHook(() => useKeyboardShortcuts());
    fireKey('c');
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
  });

  it('focuses search bar when "/" is pressed', () => {
    const mockInput = document.createElement('input');
    const focusSpy = vi.spyOn(mockInput, 'focus');

    // Added: Insert a mock search input matching the selector
    const container = document.createElement('div');
    container.className = 'topbar__search';
    container.appendChild(mockInput);
    document.body.appendChild(container);

    renderHook(() => useKeyboardShortcuts());
    fireKey('/');

    expect(focusSpy).toHaveBeenCalled();
    document.body.removeChild(container);
  });

  it('navigates to next message with "j"', () => {
    const messageUids = [100, 200, 300];
    mockState.selectedUid = 100;

    renderHook(() => useKeyboardShortcuts(messageUids));
    fireKey('j');

    expect(mockSetSelectedUid).toHaveBeenCalledWith(200);
  });

  it('navigates to previous message with "k"', () => {
    const messageUids = [100, 200, 300];
    mockState.selectedUid = 200;

    renderHook(() => useKeyboardShortcuts(messageUids));
    fireKey('k');

    expect(mockSetSelectedUid).toHaveBeenCalledWith(100);
  });

  it('returns to list view when "u" is pressed', () => {
    mockState.viewMode = 'reader';
    renderHook(() => useKeyboardShortcuts());
    fireKey('u');
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('sets compose mode for "r" when in reader view', () => {
    mockState.viewMode = 'reader';
    renderHook(() => useKeyboardShortcuts());
    fireKey('r');
    expect(mockSetViewMode).toHaveBeenCalledWith('compose');
  });

  it('does not set compose mode for "r" when in list view', () => {
    mockState.viewMode = 'list';
    renderHook(() => useKeyboardShortcuts());
    fireKey('r');
    expect(mockSetViewMode).not.toHaveBeenCalled();
  });

  it('returns to list view when Escape is pressed in reader', () => {
    mockState.viewMode = 'reader';
    renderHook(() => useKeyboardShortcuts());
    fireKey('Escape');
    expect(mockSetViewMode).toHaveBeenCalledWith('list');
  });

  it('toggles help dialog when "?" is pressed', () => {
    const { result } = renderHook(() => useKeyboardShortcuts());
    expect(result.current.showHelp).toBe(false);

    act(() => { fireKey('?'); });
    expect(result.current.showHelp).toBe(true);

    act(() => { fireKey('?'); });
    expect(result.current.showHelp).toBe(false);
  });

  it('navigates to Inbox with "g" then "i" combo', () => {
    vi.useFakeTimers();
    renderHook(() => useKeyboardShortcuts());

    fireKey('g');
    fireKey('i');

    expect(mockSetSelectedFolder).toHaveBeenCalledWith('INBOX');
    vi.useRealTimers();
  });

  it('navigates to Sent with "g" then "s" combo', () => {
    vi.useFakeTimers();
    renderHook(() => useKeyboardShortcuts());

    fireKey('g');
    fireKey('s');

    expect(mockSetSelectedFolder).toHaveBeenCalledWith('Sent');
    vi.useRealTimers();
  });

  it('navigates to Drafts with "g" then "d" combo', () => {
    vi.useFakeTimers();
    renderHook(() => useKeyboardShortcuts());

    fireKey('g');
    fireKey('d');

    expect(mockSetSelectedFolder).toHaveBeenCalledWith('Drafts');
    vi.useRealTimers();
  });

  it('deletes selected message when "#" is pressed', async () => {
    mockState.selectedUid = 42;
    mockState.selectedFolder = 'INBOX';

    renderHook(() => useKeyboardShortcuts());
    fireKey('#');

    expect(deleteMessage).toHaveBeenCalledWith('INBOX', 42);
  });

  it('archives selected message when "e" is pressed', () => {
    mockState.selectedUid = 42;
    mockState.selectedFolder = 'INBOX';

    renderHook(() => useKeyboardShortcuts());
    fireKey('e');

    expect(moveMessage).toHaveBeenCalledWith('INBOX', 42, 'Archive');
  });

  it('stars selected message when "s" is pressed', () => {
    mockState.selectedUid = 42;

    renderHook(() => useKeyboardShortcuts());
    fireKey('s');

    expect(flagMessage).toHaveBeenCalledWith('INBOX', 42, '\\Flagged', true);
  });

  it('cleans up event listener on unmount', () => {
    const removeSpy = vi.spyOn(document, 'removeEventListener');
    const { unmount } = renderHook(() => useKeyboardShortcuts());

    unmount();

    expect(removeSpy).toHaveBeenCalledWith('keydown', expect.any(Function));
    removeSpy.mockRestore();
  });
});
