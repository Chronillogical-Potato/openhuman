import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { GoalToolLine } from './GoalToolLine';

const baseProps = {
  type: 'tool-call' as const,
  toolName: 'goal_set',
  toolCallId: 'call-1',
  argsText: '{}',
  addResult: () => {},
  resume: () => {},
  respondToApproval: async () => {},
};

describe('GoalToolLine', () => {
  it('renders "{objective} ({status})" from the result payload', () => {
    render(
      <GoalToolLine
        {...baseProps}
        args={{} as never}
        result={{ goal: { objective: 'Ship the feature', status: 'active' } } as never}
        status={{ type: 'complete' }}
      />
    );
    expect(screen.getByText('Ship the feature (active)')).toBeInTheDocument();
  });

  it('falls back to args while the call is still in flight', () => {
    render(
      <GoalToolLine
        {...baseProps}
        args={{ goal: { objective: 'Ship the feature', status: 'active' } } as never}
        result={undefined}
        status={{ type: 'running' }}
      />
    );
    expect(screen.getByText('Ship the feature (active)')).toBeInTheDocument();
  });

  it('renders nothing for a cleared goal (goal_get with no goal)', () => {
    const { container } = render(
      <GoalToolLine
        {...baseProps}
        toolName="goal_get"
        args={{} as never}
        result={{ goal: null } as never}
        status={{ type: 'complete' }}
      />
    );
    expect(container).toBeEmptyDOMElement();
  });
});
