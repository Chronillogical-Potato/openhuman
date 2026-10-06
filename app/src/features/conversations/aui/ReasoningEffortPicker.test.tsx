import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ReasoningEffortPicker, toReasoningEffortChoice } from './ReasoningEffortPicker';

vi.mock('../../../lib/i18n/I18nContext', () => ({ useT: () => ({ t: (key: string) => key }) }));

describe('ReasoningEffortPicker', () => {
  it('offers every thinking level and reports the pick', () => {
    const onChange = vi.fn();
    render(<ReasoningEffortPicker value="default" onChange={onChange} />);

    const select = screen.getByTestId('composer-reasoning-effort') as HTMLSelectElement;
    expect(select).toHaveAccessibleName('composer.reasoning.label');
    expect(Array.from(select.options).map(o => o.value)).toEqual([
      'default',
      'none',
      'low',
      'medium',
      'high',
      'xhigh',
    ]);

    fireEvent.change(select, { target: { value: 'high' } });
    expect(onChange).toHaveBeenCalledWith('high');
  });

  it('shows the current value and can be disabled', () => {
    render(<ReasoningEffortPicker value="none" onChange={vi.fn()} disabled />);
    const select = screen.getByTestId('composer-reasoning-effort') as HTMLSelectElement;
    expect(select.value).toBe('none');
    expect(select).toBeDisabled();
  });
});

describe('toReasoningEffortChoice', () => {
  it('normalizes config values and aliases', () => {
    expect(toReasoningEffortChoice('HIGH')).toBe('high');
    expect(toReasoningEffortChoice('off')).toBe('none');
    expect(toReasoningEffortChoice('max')).toBe('xhigh');
    expect(toReasoningEffortChoice('minimal')).toBe('default');
    expect(toReasoningEffortChoice(null)).toBe('default');
    expect(toReasoningEffortChoice('')).toBe('default');
  });
});
