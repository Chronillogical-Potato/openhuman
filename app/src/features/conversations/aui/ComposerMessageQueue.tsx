/**
 * The composer's message queue: the running prompt and the messages queued
 * behind it, rendered with assistant-ui's `message-queue` element.
 *
 * Reads `s.composer.queue`, the same list `ComposerPrimitive.Queue` iterates,
 * which the runtime fills from the external store's `queue` adapter
 * (`queueAdapter.ts`, over the core's run queue). The element owns the whole
 * list, so it takes the array rather than rendering one primitive per item.
 * Removal goes back through the runtime (`composer.queueItem().remove()`), so
 * it lands on the adapter and from there on the core.
 */
import { useAui, useAuiState } from '@assistant-ui/react';

import { MessageQueue } from '../../../components/assistant-ui/elements/message-queue';
import { useT } from '../../../lib/i18n/I18nContext';

type ThreadMessages = ReadonlyArray<{
  role: string;
  content: ReadonlyArray<{ type: string; text?: string }>;
}>;

/** Text of the newest user message: the prompt the running turn answers. */
function runningPrompt(messages: ThreadMessages): string {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const message = messages[i];
    if (message.role !== 'user') continue;
    return message.content
      .map(part => (part.type === 'text' ? (part.text ?? '') : ''))
      .join('')
      .trim();
  }
  return '';
}

export function ComposerMessageQueue() {
  const { t } = useT();
  const aui = useAui();
  const queue = useAuiState(s => s.composer.queue);
  const running = useAuiState(s => runningPrompt(s.thread.messages as ThreadMessages));

  if (queue.length === 0) return null;

  return (
    <MessageQueue
      data-testid="queued-followups"
      className="mb-2 max-w-none"
      running={running}
      queued={queue.map(item => ({ id: item.id, text: item.prompt }))}
      onCancel={id => aui.composer.queueItem({ id }).remove()}
      runningLabel={t('chat.messageQueue.running')}
      queuedLabel={count => t('chat.messageQueue.queuedCount').replace('{count}', String(count))}
      pendingHint={t('chat.messageQueue.pendingHint')}
      removeLabel={text => t('chat.messageQueue.remove').replace('{text}', text)}
    />
  );
}
