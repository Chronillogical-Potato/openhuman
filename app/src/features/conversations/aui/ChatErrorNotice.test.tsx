import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { Thread } from '../../../components/assistant-ui/thread';
import { toThreadMessageLike } from '../../../providers/assistantUiMessages';
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
              custom: { extraMetadata: { [CHAT_ERROR_METADATA_KEY]: { errorType: 'timeout' } } },
            },
          },
        ]}
      />
    );

    expect(screen.queryByTestId('assistant-ui-guardrail-notice')).not.toBeInTheDocument();
  });

  it('renders a persisted chat failure as an error card without legacy link markup', () => {
    const message = toThreadMessageLike({
      id: 'failed-turn',
      sender: 'agent',
      type: 'text',
      content:
        'Something went wrong. Please try again.\n<openhuman-link path="community/discord-report">Report on Discord</openhuman-link>',
      createdAt: '2026-01-01T00:00:00.000Z',
      extraMetadata: { [CHAT_ERROR_METADATA_KEY]: { errorType: 'inference' } },
    });

    render(<Harness messages={[message]} />);

    const card = screen.getByRole('alert');
    expect(card).toHaveAttribute('data-slot', 'error-state');
    expect(card).toHaveTextContent('Something went wrong. Please try again.');
    expect(card).not.toHaveTextContent('openhuman-link');
  });
});
