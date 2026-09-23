/**
 * Read-aloud, proven from the component `/chat` actually mounts.
 *
 * `ActionBarPrimitive.Speak` has the same trap as Reload: its disabled
 * predicate checks only the message's role and running status, NOT
 * `capabilities.speech`. So a Speak button rendered against a runtime with no
 * `adapters.speech` is enabled, clickable, and throws. Both halves have to be
 * present for the control to mean anything, which is what these tests assert —
 * the button appears only when the runtime can actually speak.
 */
import {
  AssistantRuntimeProvider,
  type SpeechSynthesisAdapter,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { Thread } from '../thread';

const messages: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: 'explain mitochondria' }] },
  {
    role: 'assistant',
    content: [{ type: 'text', text: 'The mitochondria is the powerhouse of the cell.' }],
  },
];

/** A speech adapter whose utterance this test drives by hand. */
function controllableSpeech() {
  const cancel = vi.fn();
  const speak = vi.fn((_text: string) => {
    const listeners = new Set<() => void>();
    const utterance = {
      status: { type: 'running' } as SpeechSynthesisAdapter.Utterance['status'],
      cancel: () => {
        cancel();
        utterance.status = { type: 'ended', reason: 'cancelled' };
        for (const l of listeners) l();
      },
      subscribe: (cb: () => void) => {
        listeners.add(cb);
        return () => listeners.delete(cb);
      },
    };
    return utterance;
  });
  return { adapter: { speak } as unknown as SpeechSynthesisAdapter, speak, cancel };
}

function Harness({ speech }: { speech?: SpeechSynthesisAdapter }) {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning: false,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
    ...(speech ? { adapters: { speech } } : {}),
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread model={null} onModelChange={() => {}} />
    </AssistantRuntimeProvider>
  );
}

describe('read-aloud on the live Thread', () => {
  it('offers Read aloud when the runtime can speak', async () => {
    const { adapter } = controllableSpeech();
    render(<Harness speech={adapter} />);

    await screen.findByText('The mitochondria is the powerhouse of the cell.');
    expect(await screen.findByRole('button', { name: /read aloud/i })).toBeInTheDocument();
  });

  it('offers NO Read aloud button when the runtime cannot speak', async () => {
    // The guard against the Reload defect: without an adapter the control must
    // be absent, not present-and-throwing. `capabilities.speech` is false here
    // because no `adapters.speech` was supplied.
    render(<Harness />);

    await screen.findByText('The mitochondria is the powerhouse of the cell.');
    expect(screen.queryByRole('button', { name: /read aloud/i })).toBeNull();
  });

  it('speaks the message text and swaps to Stop reading', async () => {
    const { adapter, speak } = controllableSpeech();
    render(<Harness speech={adapter} />);
    await screen.findByText('The mitochondria is the powerhouse of the cell.');

    fireEvent.click(await screen.findByRole('button', { name: /read aloud/i }));

    await waitFor(() => expect(speak).toHaveBeenCalled());
    // The message's own text, not a placeholder — an adapter handed the wrong
    // string would read the wrong message aloud.
    expect(speak.mock.calls[0]?.[0]).toContain('powerhouse of the cell');
    // While it is speaking the affordance must become Stop, or there is no way
    // to silence it.
    expect(await screen.findByRole('button', { name: /stop reading/i })).toBeInTheDocument();
  });

  it('stops the utterance when Stop reading is pressed', async () => {
    const { adapter, cancel } = controllableSpeech();
    render(<Harness speech={adapter} />);
    await screen.findByText('The mitochondria is the powerhouse of the cell.');
    fireEvent.click(await screen.findByRole('button', { name: /read aloud/i }));

    fireEvent.click(await screen.findByRole('button', { name: /stop reading/i }));

    await waitFor(() => expect(cancel).toHaveBeenCalled());
    await waitFor(() => expect(screen.queryByRole('button', { name: /stop reading/i })).toBeNull());
  });
});
