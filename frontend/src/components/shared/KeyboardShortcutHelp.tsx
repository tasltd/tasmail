import { useEffect } from 'react';
import { SHORTCUTS } from '../../hooks/useKeyboardShortcuts';

interface KeyboardShortcutHelpProps {
  onClose: () => void;
}

/**
 * PURPOSE: Modal dialog displaying all available keyboard shortcuts
 * CONSTRAINTS: Must close on Escape key and backdrop click
 */
export function KeyboardShortcutHelp({ onClose }: KeyboardShortcutHelpProps) {
  // Added: Close on Escape key within the dialog
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        onClose();
      }
    }
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [onClose]);

  return (
    <div
      className="keyboard-shortcut-help__overlay"
      onClick={onClose}
      role="presentation"
    >
      <div
        className="keyboard-shortcut-help__dialog"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-label="Keyboard shortcuts"
      >
        <div className="keyboard-shortcut-help__header">
          <h2>Keyboard Shortcuts</h2>
          <button
            className="keyboard-shortcut-help__close"
            onClick={onClose}
            aria-label="Close"
          >
            &times;
          </button>
        </div>
        <table className="keyboard-shortcut-help__table">
          <thead>
            <tr>
              <th>Shortcut</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {SHORTCUTS.map((shortcut) => (
              <tr key={shortcut.keys}>
                <td>
                  <kbd>{shortcut.keys}</kbd>
                </td>
                <td>{shortcut.description}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
