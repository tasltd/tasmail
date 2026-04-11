import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { LoadingSkeleton } from './LoadingSkeleton';

describe('LoadingSkeleton', () => {
  it('renders default 5 skeleton rows', () => {
    const { container } = render(<LoadingSkeleton />);
    const rows = container.querySelectorAll('.skeleton-row');
    expect(rows).toHaveLength(5);
  });

  it('renders custom number of rows when rows prop is provided', () => {
    const { container } = render(<LoadingSkeleton rows={3} />);
    const rows = container.querySelectorAll('.skeleton-row');
    expect(rows).toHaveLength(3);
  });

  it('renders 1 row when rows=1', () => {
    const { container } = render(<LoadingSkeleton rows={1} />);
    const rows = container.querySelectorAll('.skeleton-row');
    expect(rows).toHaveLength(1);
  });

  it('renders 0 rows when rows=0', () => {
    const { container } = render(<LoadingSkeleton rows={0} />);
    const rows = container.querySelectorAll('.skeleton-row');
    expect(rows).toHaveLength(0);
  });

  it('each row has skeleton-row class', () => {
    const { container } = render(<LoadingSkeleton rows={3} />);
    const rows = container.querySelectorAll('.skeleton-row');
    rows.forEach((row) => {
      expect(row.className).toContain('skeleton-row');
    });
  });

  it('wraps rows in loading-skeleton container', () => {
    const { container } = render(<LoadingSkeleton />);
    expect(container.querySelector('.loading-skeleton')).toBeInTheDocument();
  });
});
