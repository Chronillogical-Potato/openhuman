import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { ThreadGoalView } from '../utils/harnessState';
import { formatTokens, GoalBanner } from './GoalBanner';

// Echo i18n keys so assertions read the stable key string; the interpolated
// usage keys get their English templates so the substitution is visible.
const TEMPLATES: Record<string, string> = {
  'conversations.goal.tokens': '{used} tokens',
  'conversations.goal.tokensWithBudget': '{used} / {budget} tokens',
};
vi.mock('../../../lib/i18n/I18nContext', () => ({
  useT: () => ({ t: (key: string) => TEMPLATES[key] ?? key }),
}));

function goal(partial: Partial<ThreadGoalView> = {}): ThreadGoalView {
  return {
    goalId: 'g1',
    objective: 'Ship the v2 release',
    status: 'active',
    tokensUsed: 1200,
    tokenBudget: 50000,
    ...partial,
  };
}

describe('GoalBanner', () => {
  it('renders the objective, status, and usage against the budget', () => {
    render(<GoalBanner goal={goal()} />);
    expect(screen.getByTestId('goal-objective').textContent).toBe('Ship the v2 release');
    expect(screen.getByTestId('goal-status').textContent).toBe('conversations.goal.status.active');
    expect(screen.getByTestId('goal-tokens').textContent).toBe('1.2k / 50k tokens');
    expect(screen.getByTestId('goal-banner').getAttribute('data-goal-status')).toBe('active');
  });

  it('drops the budget half when the goal has none', () => {
    render(<GoalBanner goal={goal({ tokenBudget: null, tokensUsed: 42 })} />);
    expect(screen.getByTestId('goal-tokens').textContent).toBe('42 tokens');
  });

  it.each([
    ['paused', 'conversations.goal.status.paused'],
    ['budget_limited', 'conversations.goal.status.budgetLimited'],
    ['complete', 'conversations.goal.status.complete'],
  ] as const)('labels the %s status', (status, key) => {
    render(<GoalBanner goal={goal({ status })} />);
    expect(screen.getByTestId('goal-status').textContent).toBe(key);
    expect(screen.getByTestId('goal-banner').getAttribute('data-goal-status')).toBe(status);
  });

  it('expands a clamped objective on click', () => {
    render(<GoalBanner goal={goal()} />);
    const objective = screen.getByTestId('goal-objective');
    expect(objective.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(objective);
    expect(objective.getAttribute('aria-expanded')).toBe('true');
  });
});

describe('formatTokens', () => {
  it('abbreviates thousands and millions, keeps small counts exact', () => {
    expect(formatTokens(0)).toBe('0');
    expect(formatTokens(999)).toBe('999');
    expect(formatTokens(1000)).toBe('1k');
    expect(formatTokens(1250)).toBe('1.3k');
    expect(formatTokens(50000)).toBe('50k');
    expect(formatTokens(2_500_000)).toBe('2.5M');
  });
});
