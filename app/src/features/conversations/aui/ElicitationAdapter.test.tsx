import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ElicitationAdapter } from './ElicitationAdapter';

describe('ElicitationAdapter', () => {
  it('renders the question and lets the user type an answer', async () => {
    const onAnswer = vi.fn();
    render(
      <ElicitationAdapter server="OpenHuman" message="Which repo?" pending onAnswer={onAnswer} />
    );

    expect(screen.getByText('Which repo?')).toBeInTheDocument();
    const input = screen.getByRole('textbox');
    await userEvent.type(input, 'openhuman');
    await userEvent.click(screen.getByRole('button', { name: 'Send' }));

    expect(onAnswer).toHaveBeenCalledWith('openhuman');
  });

  it('does not call onAnswer for a blank answer', async () => {
    const onAnswer = vi.fn();
    render(<ElicitationAdapter server="OpenHuman" message="?" pending onAnswer={onAnswer} />);

    await userEvent.click(screen.getByRole('button', { name: 'Send' }));
    expect(onAnswer).not.toHaveBeenCalled();
  });

  it('shows the accepted state once the run is no longer pending', () => {
    render(
      <ElicitationAdapter server="OpenHuman" message="?" pending={false} onAnswer={vi.fn()} />
    );

    expect(screen.getByText('Sent to OpenHuman')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Send' })).not.toBeInTheDocument();
  });

  it('declines when onDecline is supplied and clicked', async () => {
    const onDecline = vi.fn();
    render(
      <ElicitationAdapter
        server="OpenHuman"
        message="?"
        pending
        onAnswer={vi.fn()}
        onDecline={onDecline}
      />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Decline' }));
    expect(onDecline).toHaveBeenCalled();
    expect(screen.getByText('Declined')).toBeInTheDocument();
  });
});
