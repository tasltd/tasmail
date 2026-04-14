// Added: AI Compose Panel tests for TMAIL-134
// PURPOSE: Verify AiComposePanel renders all UI elements correctly

import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { AiComposePanel } from './AiComposePanel';

// Added: Mock the ai-config API to prevent real HTTP calls
vi.mock('../../api/ai-config', () => ({
  composeEmail: vi.fn(),
}));

describe('AiComposePanel', () => {
  const mockOnUseDraft = vi.fn();

  it('renders prompt input', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    const promptTextarea = screen.getByTestId('ai-compose-prompt');
    expect(promptTextarea).toBeInTheDocument();
    expect(promptTextarea).toHaveAttribute('placeholder', 'Describe what you want to write...');
  });

  it('shows tone selector with all options', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    const toneSelect = screen.getByTestId('ai-compose-tone');
    expect(toneSelect).toBeInTheDocument();
    // NOTE: Verify all 4 tone options are present
    const toneOptions = toneSelect.querySelectorAll('option');
    const toneValues = Array.from(toneOptions).map((option) => option.getAttribute('value'));
    expect(toneValues).toContain('professional');
    expect(toneValues).toContain('casual');
    expect(toneValues).toContain('friendly');
    expect(toneValues).toContain('formal');
  });

  it('shows length selector with all options', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    const lengthSelect = screen.getByTestId('ai-compose-length');
    expect(lengthSelect).toBeInTheDocument();
    // NOTE: Verify all 3 length options are present
    const lengthOptions = lengthSelect.querySelectorAll('option');
    const lengthValues = Array.from(lengthOptions).map((option) => option.getAttribute('value'));
    expect(lengthValues).toContain('short');
    expect(lengthValues).toContain('medium');
    expect(lengthValues).toContain('long');
  });

  it('shows generate button', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    const generateButton = screen.getByTestId('ai-compose-generate');
    expect(generateButton).toBeInTheDocument();
    expect(generateButton).toHaveTextContent('Generate Draft');
  });

  it('does not show preview area before generation', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    // NOTE: Preview should not be visible until a draft has been generated
    expect(screen.queryByTestId('ai-compose-preview')).not.toBeInTheDocument();
    expect(screen.queryByTestId('ai-compose-use-draft')).not.toBeInTheDocument();
  });

  it('renders context textarea', () => {
    render(<AiComposePanel onUseDraft={mockOnUseDraft} />);
    const contextTextarea = screen.getByTestId('ai-compose-context');
    expect(contextTextarea).toBeInTheDocument();
    expect(contextTextarea).toHaveAttribute('placeholder', 'Additional context...');
  });
});
