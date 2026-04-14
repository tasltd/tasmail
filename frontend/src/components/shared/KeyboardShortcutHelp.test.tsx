import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { KeyboardShortcutHelp } from './KeyboardShortcutHelp';
import { SHORTCUTS } from '../../hooks/useKeyboardShortcuts';

// Added: Mock the mail store to satisfy useKeyboardShortcuts import chain
vi.mock('../../stores/mailStore', () => ({
  useMailStore: Object.assign(
    () => ({}),
    { getState: () => ({}) },
  ),
}));

vi.mock('../../api/messages', () => ({
  deleteMessage: vi.fn(),
  moveMessage: vi.fn(),
  flagMessage: vi.fn(),
}));

/**
 * PURPOSE: Unit tests for the keyboard shortcut help dialog component
 * CONSTRAINTS: Tests rendering, close button, and Escape key behavior
 */
describe('KeyboardShortcutHelp', () => {
  it('renders all shortcuts in the list', () => {
    render(<KeyboardShortcutHelp onClose={vi.fn()} />);

    for (const shortcut of SHORTCUTS) {
      expect(screen.getByText(shortcut.description)).toBeInTheDocument();
    }
  });

  it('renders the dialog with correct heading', () => {
    render(<KeyboardShortcutHelp onClose={vi.fn()} />);
    expect(screen.getByText('Keyboard Shortcuts')).toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutHelp onClose={onClose} />);

    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when Escape key is pressed', () => {
    const onClose = vi.fn();
    render(<KeyboardShortcutHelp onClose={onClose} />);

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
