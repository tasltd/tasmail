import { useEffect, useState, useRef, useCallback } from 'react';
import { useMailStore } from '../stores/mailStore';
import { deleteMessage, moveMessage, flagMessage } from '../api/messages';

/**
 * PURPOSE: Define all available keyboard shortcuts for display in help dialog
 * CONSTRAINTS: Multi-key combos (g+letter) use a pending buffer with 500ms timeout
 */
export const SHORTCUTS = [
  { keys: 'c', description: 'Compose new email' },
  { keys: '/', description: 'Focus search bar' },
  { keys: 'j', description: 'Next message (down)' },
  { keys: 'k', description: 'Previous message (up)' },
  { keys: 'o / Enter', description: 'Open selected message' },
  { keys: 'u', description: 'Back to message list' },
  { keys: 'r', description: 'Reply to message' },
  { keys: '#', description: 'Delete selected message' },
  { keys: 'e', description: 'Archive message' },
  { keys: 's', description: 'Star/unstar message' },
  { keys: 'Escape', description: 'Close / go back' },
  { keys: '?', description: 'Show keyboard shortcuts' },
  { keys: 'g then i', description: 'Go to Inbox' },
  { keys: 'g then s', description: 'Go to Sent' },
  { keys: 'g then d', description: 'Go to Drafts' },
  { keys: 'g then t', description: 'Go to Trash' },
] as const;

// Added: Map of g+key combos to folder names
const GO_TO_FOLDERS: Record<string, string> = {
  i: 'INBOX',
  s: 'Sent',
  d: 'Drafts',
  t: 'Trash',
};

/**
 * PURPOSE: Check if the active element is an input field where typing should not trigger shortcuts
 * CONSTRAINTS: Covers input, textarea, select, and contenteditable elements
 */
function isTypingTarget(target: EventTarget | null): boolean {
  if (!target || !(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  if (tagName === 'input' || tagName === 'textarea' || tagName === 'select') return true;
  if (target.isContentEditable) return true;
  return false;
}

/**
 * PURPOSE: Gmail-like keyboard shortcuts for mail navigation and actions
 * CONSTRAINTS: Skips shortcuts when user is typing in input/textarea/contenteditable
 * EXTERNAL: Uses useMailStore for state, messages API for delete/move/flag
 */
export function useKeyboardShortcuts(messageUids: number[] = []) {
  const [showHelp, setShowHelp] = useState(false);

  // NOTE: Use refs for pending key buffer to avoid re-registering the listener on every state change
  const pendingKeyRef = useRef<string | null>(null);
  const pendingTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Added: Access store actions via getState to avoid stale closures in the event handler
  const getMailState = useMailStore.getState;

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    if (isTypingTarget(event.target)) return;

    const key = event.key;
    const state = getMailState();
    const {
      viewMode,
      selectedFolder,
      selectedUid,
      setViewMode,
      setSelectedFolder,
      setSelectedUid,
    } = state;

    // Added: Handle pending multi-key combos (g + letter)
    if (pendingKeyRef.current === 'g') {
      pendingKeyRef.current = null;
      if (pendingTimerRef.current) {
        clearTimeout(pendingTimerRef.current);
        pendingTimerRef.current = null;
      }
      const folder = GO_TO_FOLDERS[key];
      if (folder) {
        setSelectedFolder(folder);
        return;
      }
      // NOTE: If no matching folder, fall through to normal key handling
    }

    // Added: Start multi-key combo if 'g' is pressed
    if (key === 'g') {
      pendingKeyRef.current = 'g';
      pendingTimerRef.current = setTimeout(() => {
        pendingKeyRef.current = null;
      }, 500);
      return;
    }

    switch (key) {
      case 'c':
        setViewMode('compose');
        break;

      case '/':
        event.preventDefault();
        (document.querySelector('.topbar__search input') as HTMLElement | null)?.focus();
        break;

      case 'j': {
        // Added: Navigate to next message in list
        if (messageUids.length === 0) break;
        const currentIndex = selectedUid !== null ? messageUids.indexOf(selectedUid) : -1;
        const nextIndex = Math.min(currentIndex + 1, messageUids.length - 1);
        setSelectedUid(messageUids[nextIndex]);
        break;
      }

      case 'k': {
        // Added: Navigate to previous message in list
        if (messageUids.length === 0) break;
        const currentIndex = selectedUid !== null ? messageUids.indexOf(selectedUid) : messageUids.length;
        const prevIndex = Math.max(currentIndex - 1, 0);
        setSelectedUid(messageUids[prevIndex]);
        break;
      }

      case 'o':
      case 'Enter':
        // Added: Open selected message in reader view
        if (selectedUid !== null && viewMode === 'list') {
          setSelectedUid(selectedUid);
        }
        break;

      case 'u':
        setViewMode('list');
        break;

      case 'r':
        // Added: Reply shortcut only works when viewing a message
        if (viewMode === 'reader') {
          setViewMode('compose');
        }
        break;

      case '#':
        // Added: Delete currently selected message
        if (selectedUid !== null) {
          deleteMessage(selectedFolder, selectedUid).then(() => {
            setSelectedUid(null);
          });
        }
        break;

      case 'e':
        // Added: Archive (move to Archive folder)
        if (selectedUid !== null) {
          moveMessage(selectedFolder, selectedUid, 'Archive').then(() => {
            setSelectedUid(null);
          });
        }
        break;

      case 's':
        // Added: Toggle star/flag on selected message
        if (selectedUid !== null) {
          flagMessage(selectedFolder, selectedUid, '\\Flagged', true);
        }
        break;

      case 'Escape':
        // Added: Close help dialog, or go back to list from reader/compose
        if (showHelp) {
          setShowHelp(false);
        } else if (viewMode === 'reader' || viewMode === 'compose') {
          setViewMode('list');
        }
        break;

      case '?':
        setShowHelp((prev) => !prev);
        break;

      default:
        break;
    }
  }, [messageUids, showHelp, getMailState]);

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      if (pendingTimerRef.current) {
        clearTimeout(pendingTimerRef.current);
      }
    };
  }, [handleKeyDown]);

  return { showHelp, setShowHelp };
}
