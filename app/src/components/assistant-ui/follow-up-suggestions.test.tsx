import {
  AppendMessage,
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  type ThreadSuggestion,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { ThreadFollowupSuggestions } from './follow-up-suggestions';

const settled: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: 'What is on today?' }] },
  { role: 'assistant', content: [{ type: 'text', text: 'Two meetings.' }] },
];

const SUGGESTIONS: ThreadSuggestion[] = [
  { prompt: 'Move the second meeting to Friday', title: 'Reschedule' },
  { prompt: 'Who is attending?' },
];

function Harness({
  messages = settled,
  isRunning = false,
  onNew = async () => {},
}: {
  messages?: ThreadMessageLike[];
  isRunning?: boolean;
  onNew?: (m: AppendMessage) => Promise<void>;
}) {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning,
    suggestions: SUGGESTIONS,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew,
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ThreadFollowupSuggestions />
    </AssistantRuntimeProvider>
  );
}

describe('ThreadFollowupSuggestions', () => {
  it('renders one chip per suggestion, titled by its title or else its prompt', () => {
    render(<Harness />);

    expect(screen.getByRole('button', { name: 'Reschedule' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Who is attending?' })).toBeTruthy();
  });

  it('sends the chip prompt, not its title, on click', async () => {
    const onNew = vi.fn(async (_m: AppendMessage) => {});
    render(<Harness onNew={onNew} />);

    fireEvent.click(screen.getByRole('button', { name: 'Reschedule' }));

    await waitFor(() => expect(onNew).toHaveBeenCalledTimes(1));
    expect(onNew.mock.calls[0][0].content).toEqual([
      { type: 'text', text: 'Move the second meeting to Friday' },
    ]);
  });

  it('renders nothing while a turn runs or on an empty thread', () => {
    const running = render(<Harness isRunning />);
    expect(running.queryAllByRole('button')).toHaveLength(0);
    running.unmount();

    const empty = render(<Harness messages={[]} />);
    expect(empty.queryAllByRole('button')).toHaveLength(0);
  });
});
