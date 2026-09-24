/**
 * A tool card's disclosure must look the same live and on reload, and must not
 * reset under the reader.
 *
 * It used to be `defaultOpen={running}`: a card mounted while its call ran
 * opened and stayed open, the same card reloaded (already settled) was closed,
 * and any remount in between flipped it — the "collapsing" part of the live
 * thread glitching.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { AssistantUiToolCallCard } from './AssistantUiToolCall';

function card() {
  return screen.getByTestId('assistant-ui-tool-call');
}

describe('AssistantUiToolCallCard disclosure', () => {
  it('starts collapsed whether it mounts running (live) or settled (reload)', () => {
    const { unmount } = render(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} disclosureKey="live" />
    );
    expect(card()).toHaveAttribute('data-state', 'closed');
    unmount();

    render(
      <AssistantUiToolCallCard
        toolName="search"
        args={{ q: 'x' }}
        result="done"
        disclosureKey="reloaded"
      />
    );
    expect(card()).toHaveAttribute('data-state', 'closed');
  });

  it('does not change state when the call settles', () => {
    const { rerender } = render(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} disclosureKey="c1" />
    );
    rerender(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} result="ok" disclosureKey="c1" />
    );
    expect(card()).toHaveAttribute('data-state', 'closed');
  });

  it('remembers the reader’s choice across a remount of the same call', () => {
    const { unmount } = render(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} disclosureKey="c2" />
    );
    fireEvent.click(screen.getByRole('button'));
    expect(card()).toHaveAttribute('data-state', 'open');
    unmount();

    render(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} result="ok" disclosureKey="c2" />
    );
    expect(card()).toHaveAttribute('data-state', 'open');
  });

  it('does not leak one call’s choice onto another', () => {
    const { unmount } = render(
      <AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} disclosureKey="c3" />
    );
    fireEvent.click(screen.getByRole('button'));
    unmount();

    render(<AssistantUiToolCallCard toolName="search" args={{ q: 'x' }} disclosureKey="c4" />);
    expect(card()).toHaveAttribute('data-state', 'closed');
  });
});
