/**
 * The reasoning block's automatic collapse must not take the scroll lock.
 *
 * `useScrollLock` pins `scrollTop` by writing it back on every scroll event for
 * the animation's length. When streaming ended and the block auto-collapsed, it
 * did that while the thread was following the reply to the bottom: the follower
 * scrolled down, the lock dragged it back, and the follower read the drop as
 * the reader leaving — following stopped for the rest of the turn. The lock is
 * for a panel the READER toggles.
 */
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const lockScroll = vi.fn();

vi.mock('@assistant-ui/react', async () => {
  const actual = await vi.importActual<typeof import('@assistant-ui/react')>('@assistant-ui/react');
  return { ...actual, useScrollLock: () => lockScroll };
});

const { ReasoningRoot, ReasoningTrigger } = await import('../reasoning');

function reasoning(streaming: boolean) {
  return (
    <ReasoningRoot streaming={streaming}>
      <ReasoningTrigger active={streaming} />
    </ReasoningRoot>
  );
}

describe('ReasoningRoot scroll lock', () => {
  beforeEach(() => lockScroll.mockClear());

  it('does not lock when streaming ends and the block collapses on its own', () => {
    const { rerender } = render(reasoning(true));
    rerender(reasoning(false));
    expect(lockScroll).not.toHaveBeenCalled();
  });

  it('locks when the reader toggles it', () => {
    render(reasoning(false));
    fireEvent.click(screen.getByRole('button'));
    expect(lockScroll).toHaveBeenCalledTimes(1);
  });
});
