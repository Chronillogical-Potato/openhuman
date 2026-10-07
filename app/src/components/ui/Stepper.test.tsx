import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import Stepper from './Stepper';

const labels = ['One', 'Two', 'Three'];

describe('Stepper', () => {
  it('renders labels in order under the given aria-label', () => {
    render(<Stepper labels={labels} activeIndex={0} ariaLabel="Progress" data-testid="s" />);
    expect(screen.getByRole('list', { name: 'Progress' })).toBe(screen.getByTestId('s'));
    expect(screen.getAllByRole('listitem').map(li => li.textContent)).toEqual([
      '1One',
      '2Two',
      '3Three',
    ]);
  });

  it('marks only the active step as current', () => {
    render(<Stepper labels={labels} activeIndex={1} ariaLabel="Progress" />);
    const items = screen.getAllByRole('listitem');
    expect(items[1]).toHaveAttribute('aria-current', 'step');
    expect(items[0]).not.toHaveAttribute('aria-current');
    expect(items[2]).not.toHaveAttribute('aria-current');
  });

  it('shows a check for completed steps and a number otherwise', () => {
    render(<Stepper labels={labels} activeIndex={2} ariaLabel="Progress" />);
    const items = screen.getAllByRole('listitem');
    expect(items[0].querySelector('svg')).not.toBeNull();
    expect(items[1].querySelector('svg')).not.toBeNull();
    expect(items[2].querySelector('svg')).toBeNull();
    expect(items[2].textContent).toContain('3');
  });
});
