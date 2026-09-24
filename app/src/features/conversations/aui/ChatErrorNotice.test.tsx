import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Thread } from '../../../components/assistant-ui/thread';
import { CHAT_ERROR_METADATA_KEY } from '../../../store/threadSlice';

/**
 * Mirrors `thread.directiveText.test.tsx`'s harness: driven through `Thread`
 * on `useExternalStoreRuntime` (the runtime family `/chat` uses), not the
 * dev demo, so this exercises the real `AssistantMessage` render path where
 * `ChatErrorNotice` is mounted.
 */
function Harness({ messages }: { messages: ThreadMessageLike[] }) {
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

const guardrailMessage: ThreadMessageLike = {
  role: 'assistant',
  content: [],
  metadata: {
    custom: {
      extraMetadata: {
        [CHAT_ERROR_METADATA_KEY]: {
          errorType: 'guardrail',
          guardrail: {
            verdict: 'blocked',
            score: 0.92,
            reasons: [{ code: 'pii_exfiltration', message: 'The reply contained a customer SSN.' }],
          },
        },
      },
    },
  },
};

describe('ChatErrorNotice', () => {
  it('renders the guardrail card with the verdict and reasons for a guardrail chat_error', () => {
    render(<Harness messages={[guardrailMessage]} />);

    const card = screen.getByTestId('assistant-ui-guardrail-notice');
    expect(card).toHaveTextContent('blocked');
    expect(card).toHaveTextContent(/customer SSN/);
  });

  it('renders nothing for an ordinary assistant message', () => {
    render(
      <Harness
        messages={[{ role: 'assistant', content: [{ type: 'text', text: 'Hello there' }] }]}
      />
    );

    expect(screen.queryByTestId('assistant-ui-guardrail-notice')).not.toBeInTheDocument();
  });

  it('renders nothing for a non-guardrail chat_error', () => {
    render(
      <Harness
        messages={[
          {
            role: 'assistant',
            content: [{ type: 'text', text: 'Something went wrong.' }],
            metadata: {
              custom: {
                extraMetadata: { [CHAT_ERROR_METADATA_KEY]: { errorType: 'timeout' } },
              },
            },
          },
        ]}
      />
    );

    expect(screen.queryByTestId('assistant-ui-guardrail-notice')).not.toBeInTheDocument();
  });
});
