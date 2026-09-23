import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Thread } from './thread';

/**
 * The Refresh button must not render while the runtime cannot honour a reload.
 *
 * assistant-ui derives the Reload primitive's disabled state from
 * `isRunning || isDisabled || role !== 'assistant'` and never consults
 * `capabilities.reload`, so the button was enabled on every settled assistant
 * message. `useOpenHumanExternalStore` supplies no `onReload`, and
 * `ExternalStoreThreadRuntimeCore.reload` throws
 * `Runtime does not support reloading messages.` — a clickable control whose
 * only outcome is an exception.
 *
 * Both directions are asserted against the SAME `<Thread />`, toggled only by
 * whether the adapter passes `onReload`, so the test fails if the gate is
 * removed AND if it is hard-coded off.
 */

const messages: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: 'hello' }] },
  { role: 'assistant', content: [{ type: 'text', text: 'a reply' }] },
];

function Harness({ withReload }: { withReload: boolean }) {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning: false,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
    ...(withReload ? { onReload: async () => {} } : {}),
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread />
    </AssistantRuntimeProvider>
  );
}

describe('assistant action bar reload gate', () => {
  it('renders no Refresh button when the runtime cannot reload', () => {
    render(<Harness withReload={false} />);
    expect(screen.getAllByTestId('agent-message').length).toBeGreaterThan(0);
    expect(screen.queryAllByRole('button', { name: 'Refresh' })).toHaveLength(0);
  });

  it('renders the Refresh button when the runtime can reload', () => {
    render(<Harness withReload />);
    expect(screen.getAllByRole('button', { name: 'Refresh' }).length).toBeGreaterThan(0);
  });
});
