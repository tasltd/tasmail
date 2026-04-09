import { describe, it, expect, beforeEach } from 'vitest';
import { useMailStore } from './mailStore';

describe('mailStore', () => {
  beforeEach(() => {
    // Reset store to defaults
    useMailStore.setState({
      selectedFolder: 'INBOX',
      selectedUid: null,
      viewMode: 'list',
      searchQuery: '',
    });
  });

  it('defaults to INBOX folder, list view, no search', () => {
    const state = useMailStore.getState();
    expect(state.selectedFolder).toBe('INBOX');
    expect(state.selectedUid).toBeNull();
    expect(state.viewMode).toBe('list');
    expect(state.searchQuery).toBe('');
  });

  it('setSelectedFolder resets uid and viewMode', () => {
    const store = useMailStore.getState();
    store.setSelectedUid(42);
    store.setSelectedFolder('Sent');
    const state = useMailStore.getState();
    expect(state.selectedFolder).toBe('Sent');
    expect(state.selectedUid).toBeNull();
    expect(state.viewMode).toBe('list');
  });

  it('setSelectedUid switches to reader mode', () => {
    const store = useMailStore.getState();
    store.setSelectedUid(10);
    const state = useMailStore.getState();
    expect(state.selectedUid).toBe(10);
    expect(state.viewMode).toBe('reader');
  });

  it('setSelectedUid(null) returns to list mode', () => {
    const store = useMailStore.getState();
    store.setSelectedUid(10);
    store.setSelectedUid(null);
    const state = useMailStore.getState();
    expect(state.selectedUid).toBeNull();
    expect(state.viewMode).toBe('list');
  });

  it('setSearchQuery switches to search mode', () => {
    const store = useMailStore.getState();
    store.setSearchQuery('invoice');
    const state = useMailStore.getState();
    expect(state.searchQuery).toBe('invoice');
    expect(state.viewMode).toBe('search');
  });

  it('setSearchQuery with empty string returns to list mode', () => {
    const store = useMailStore.getState();
    store.setSearchQuery('test');
    store.setSearchQuery('');
    const state = useMailStore.getState();
    expect(state.searchQuery).toBe('');
    expect(state.viewMode).toBe('list');
  });

  it('setViewMode changes viewMode directly', () => {
    const store = useMailStore.getState();
    store.setViewMode('compose');
    expect(useMailStore.getState().viewMode).toBe('compose');
  });
});
