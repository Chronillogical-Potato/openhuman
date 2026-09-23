import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { reasoningPart } from '../../providers/assistantUiMessages';
import { reasoningTimingOf } from './reasoning-group';
import { Thread } from './thread';

/**
 * The thread's reasoning block, driven through `Thread` on
 * `useExternalStoreRuntime` — the runtime family `/chat` uses — with parts
 * built by the same `reasoningPart` the store projection emits. Pins that:
 * - a run of reasoning parts renders as ONE static reasoning panel,
 * - its settled label is "Thought for Ns" from the parts' own timing,
 * - the steps are titled from the trace's headings,
 * - it streams open with the newest heading and collapses once settled.
 */

const T0 = Date.parse('2026-09-24T10:00:00Z');

function Harness({ messages, running }: { messages: ThreadMessageLike[]; running: boolean }) {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning: running,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread />
    </AssistantRuntimeProvider>
  );
}

const user: ThreadMessageLike = { role: 'user', content: [{ type: 'text', text: 'fix it' }] };

function assistant(running: boolean): ThreadMessageLike {
  return {
    id: 'a1',
    role: 'assistant',
    content: [
      reasoningPart('**Reading the request**\nThe user wants a fix.', T0, T0 + 4_000),
      reasoningPart('**Planning the fix**\nPatch the parser.', T0 + 5_000, T0 + 12_000),
      { type: 'text', text: 'Done.' },
    ],
    status: running ? { type: 'running' } : { type: 'complete', reason: 'stop' },
  };
}

describe('thread reasoning panel', () => {
  it('renders one collapsed "Thought for Ns" panel for a settled run of reasoning parts', () => {
    render(<Harness messages={[user, assistant(false)]} running={false} />);

    const panels = screen.getAllByTestId('reasoning-panel');
    expect(panels).toHaveLength(1);
    const panel = panels[0]!;
    expect(panel.getAttribute('data-variant')).toBe('collapsible');
    expect(panel.querySelector('[data-swap-layer="resting"]')?.textContent).toBe(
      'Thought for 12s'
    );
    const trigger = panel.querySelector('button')!;
    expect(trigger.getAttribute('aria-expanded')).toBe('false');

    fireEvent.click(trigger);
    const titles = [...panel.querySelectorAll('[data-slot="reasoning-step-title"]')].map(
      el => el.textContent
    );
    expect(titles).toEqual(['Reading the request', 'Planning the fix']);
    // The answer still renders after the reasoning block.
    expect(screen.getByText('Done.')).toBeTruthy();
  });

  it('streams open with the newest heading as its live label', () => {
    render(<Harness messages={[user, assistant(true)]} running />);
    const panel = screen.getByTestId('reasoning-panel');
    expect(panel.querySelector('button')?.getAttribute('aria-expanded')).toBe('true');
    expect(panel.querySelector('[data-slot="reasoning-panel-live-label"]')?.textContent).toBe(
      'Planning the fix'
    );
  });

  it('falls back to "Thought" for reasoning parts that carry no timing', () => {
    const legacy: ThreadMessageLike = {
      id: 'a2',
      role: 'assistant',
      content: [
        { type: 'reasoning', text: 'Old thread reasoning.' },
        { type: 'text', text: 'ok' },
      ],
    };
    render(<Harness messages={[user, legacy]} running={false} />);
    expect(
      screen.getByTestId('reasoning-panel').querySelector('[data-swap-layer="resting"]')
        ?.textContent
    ).toBe('Thought');
  });
});

describe('reasoningTimingOf', () => {
  it('reads timing back from what reasoningPart writes', () => {
    const part = reasoningPart('x', 1, 2) as Parameters<typeof reasoningTimingOf>[0];
    expect(reasoningTimingOf(part)).toEqual({ startedAt: 1, endedAt: 2 });
  });

  it('ignores missing or malformed metadata', () => {
    expect(reasoningTimingOf(undefined)).toBeUndefined();
    expect(reasoningTimingOf({ type: 'reasoning', text: 'x' })).toBeUndefined();
    expect(
      reasoningTimingOf({
        type: 'reasoning',
        providerMetadata: { openhuman: { startedAt: 'soon' } },
      })
    ).toBeUndefined();
  });
});
