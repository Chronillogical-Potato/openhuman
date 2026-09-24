import type { AppendMessage, ExternalThreadQueueAdapter } from '@assistant-ui/react';

import type { RunQueueItem } from '../../../store/queueSlice';

export function buildOpenHumanQueueAdapter(_args: {
  items: readonly RunQueueItem[];
  send: (message: AppendMessage) => Promise<void>;
  remove: (itemId: string) => void;
}): ExternalThreadQueueAdapter {
  const noop = () => {};
  return { items: [], steerItems: [], enqueue: noop, steer: noop, move: noop, edit: noop, remove: noop };
}

export function useOpenHumanQueueAdapter(
  _threadId: string | null,
  _send: (message: AppendMessage) => Promise<void>
): ExternalThreadQueueAdapter {
  return buildOpenHumanQueueAdapter({ items: [], send: _send, remove: () => {} });
}
