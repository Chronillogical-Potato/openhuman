import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
  unstable_useTriggerPopoverRootContextOptional,
} from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Thread } from './thread';

/**
 * The composer's `ComposerTriggers` slot: a host component mounted inside the
 * composer's trigger-popover root, in place of the built-in `/` popover.
 */
function Harness({ components }: { components?: Parameters<typeof Thread>[0]['components'] }) {
  const messages: ThreadMessageLike[] = [];
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread components={components} />
    </AssistantRuntimeProvider>
  );
}

function HostTriggers() {
  const root = unstable_useTriggerPopoverRootContextOptional();
  return <div data-testid="host-triggers" data-in-root={root ? 'yes' : 'no'} />;
}

describe('thread composer triggers slot', () => {
  it('mounts the host triggers inside the composer trigger-popover root', () => {
    render(<Harness components={{ ComposerTriggers: HostTriggers }} />);
    expect(screen.getByTestId('host-triggers')).toHaveAttribute('data-in-root', 'yes');
  });

  it('renders nothing extra without the slot', () => {
    render(<Harness />);
    expect(screen.queryByTestId('host-triggers')).toBeNull();
  });
});
