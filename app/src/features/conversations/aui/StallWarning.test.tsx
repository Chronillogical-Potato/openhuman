import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { StallWarning } from './StallWarning';

vi.mock('../../../lib/i18n/I18nContext', () => ({
  useT: () => ({
    t: (key: string) => (key === 'chat.status.quietFor' ? 'Quiet for {duration}.' : key),
  }),
}));

describe('StallWarning', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 7, 12, 0, 0));
  });
  afterEach(() => vi.useRealTimers());

  it('shows the phase copy, a ticking quiet duration and a Stop action', () => {
    const onStop = vi.fn();
    render(<StallWarning phase="thinking" quietSince={Date.now() - 120_000} onStop={onStop} />);

    const warning = screen.getByTestId('chat-stall-warning');
    expect(warning).toHaveAttribute('data-chat-stall-phase', 'thinking');
    expect(warning).toHaveTextContent('chat.stallWarning.thinking');
    expect(screen.getByTestId('chat-stall-quiet')).toHaveTextContent('Quiet for 2m 0s.');

    act(() => {
      vi.advanceTimersByTime(5_000);
    });
    expect(screen.getByTestId('chat-stall-quiet')).toHaveTextContent('Quiet for 2m 5s.');

    fireEvent.click(screen.getByTestId('chat-stall-stop'));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it('uses the working copy for a tool phase', () => {
    render(<StallWarning phase="tool_use" quietSince={Date.now()} onStop={vi.fn()} />);
    expect(screen.getByTestId('chat-stall-warning')).toHaveTextContent(
      'chat.stallWarning.working'
    );
  });
});
