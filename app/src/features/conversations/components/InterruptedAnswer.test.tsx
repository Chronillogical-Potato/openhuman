import { fireEvent, render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it } from 'vitest';

import { store } from '../../../store';
import { InterruptedAnswer } from './InterruptedAnswer';

function renderInStore(ui: React.ReactNode) {
  return render(<Provider store={store}>{ui}</Provider>);
}

describe('InterruptedAnswer', () => {
  it('surfaces the partial reply with an interrupted marker', () => {
    renderInStore(
      <InterruptedAnswer content="Here is the partial answer" thinking="was reasoning about it" />
    );

    const block = screen.getByTestId('interrupted-answer');
    expect(block.textContent).toContain('Here is the partial answer');
    // The reasoning it had streamed is kept in a collapsed reasoning panel
    // (settled, no timing on an interrupted buffer → "Thought").
    const thinking = screen.getByTestId('interrupted-answer-thinking');
    const trigger = thinking.querySelector('button')!;
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    expect(thinking.querySelector('[data-slot="reasoning-panel-resting-label"]')?.textContent).toBe('Thought');
    fireEvent.click(trigger);
    expect(block.textContent).toContain('was reasoning about it');
    // Marked interrupted rather than presented as a finished answer.
    expect(screen.getByTestId('interrupted-answer-marker')).toBeTruthy();
  });

  it('renders nothing when neither content nor thinking has any text', () => {
    const { container } = renderInStore(<InterruptedAnswer content="   " thinking="" />);
    expect(container.querySelector('[data-testid="interrupted-answer"]')).toBeNull();
  });
});
