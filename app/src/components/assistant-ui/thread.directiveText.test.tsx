import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  unstable_defaultDirectiveFormatter,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Thread } from './thread';

/**
 * A slash command leaves an audit-trail chip in the composer, and that chip is
 * directive syntax — so a user message can contain it and something has to
 * render it.
 *
 * The live composer produces this without anyone opting in. `thread.tsx` builds
 * its `/` popover from `unstable_useSlashCommandAdapter`, which returns an
 * `action` behaviour and does not set `removeOnExecute`. In the runtime's
 * `triggerSelectionResource`, an action whose `removeOnExecute` is falsy takes
 * `else insertDirective()` — the typed `/clear` is *replaced* by
 * `formatter.serialize(item)` rather than removed. With the default formatter
 * (`:type[label]{name=id}`) and `toItem`'s `label = '/' + id`, selecting the one
 * registered command (`clear`, globalActions.ts) writes
 * `:command[/clear]{name=clear}` into the composer, which the user then sends.
 *
 * Without a `Text` part component on the user message that text renders as raw
 * syntax. The fixture below is the serializer's own output rather than a
 * hand-written string, so this cannot drift from what the composer actually
 * inserts if upstream changes the syntax.
 *
 * Driven through `Thread` on `useExternalStoreRuntime` — the runtime family
 * `/chat` uses — rather than through the demo, which runs
 * `useRemoteThreadListRuntime` and proves nothing about the live route.
 */
const DIRECTIVE = unstable_defaultDirectiveFormatter.serialize({
  type: 'command',
  label: '/clear',
  id: 'clear',
});

const messages: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: `${DIRECTIVE} please` }] },
];

function Harness() {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning: false,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread />
    </AssistantRuntimeProvider>
  );
}

describe('directive syntax in a user message', () => {
  it('renders as a chip rather than raw syntax', () => {
    const { container } = render(<Harness />);

    const chip = container.querySelector('[data-slot="directive-text-chip"]');
    expect(
      chip,
      'the composer inserts directive syntax, so it must render as a chip'
    ).not.toBeNull();
    expect(chip?.getAttribute('data-directive-type')).toBe('command');
    expect(chip?.getAttribute('data-directive-id')).toBe('clear');
    expect(chip?.textContent).toContain('/clear');

    // The surrounding prose survives, and the raw syntax is gone from the DOM.
    expect(container.textContent).toContain('please');
    expect(container.textContent).not.toContain(':command[');
  });
});
