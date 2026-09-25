import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import CostSummary from './CostSummary';

describe('<CostSummary />', () => {
  it('shows recorded costs without implying an enforced budget', () => {
    render(
      <CostSummary
        currency="USD"
        periodTotalUsd={42.5}
        monthlyPaceUsd={181.25}
        monthToDateUsd={50}
      />
    );
    expect(screen.getByTestId('metric-total-spend')).toHaveTextContent('$42.50');
    expect(screen.getByTestId('metric-total-spend')).toHaveTextContent('$50.00');
    expect(screen.getByTestId('metric-monthly-pace')).toHaveTextContent('$181');
    expect(screen.queryByTestId('metric-budget-limit')).not.toBeInTheDocument();
    expect(screen.queryByTestId('budget-status-badge')).not.toBeInTheDocument();
  });
});
