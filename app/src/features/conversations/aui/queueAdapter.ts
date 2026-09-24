/**
 * The external-store `queue` adapter over the core's run queue.
 *
 * assistant-ui's own `createMessageQueue` keeps the queue in the browser and
 * dispatches from it. OpenHuman's queue lives in the core (`RunQueue`), so this
 * adapter is hand-rolled over it instead:
 *
 * - `items` are the core's queue items (`queueSlice`, fed by the
 *   `queue_item_*` socket events), which `ComposerPrimitive.Queue` and
 *   `s.composer.queue` read.
 * - `enqueue` and `steer` both go to the host send path. Once a runtime has a
 *   queue, assistant-ui routes every composer send through it (`steer` while
 *   running, `enqueue` when idle), and the host decides the `queue_mode`
 *   (`Conversations.handleComposerSend`: follow-up while streaming, a normal
 *   send otherwise), exactly as it did for `onNew`.
 * - `remove` asks the core to drop the item and only then drops it locally; a
 *   failed removal leaves the item showing, because the core will still send it.
 * - `move` and `edit` are no-ops: the core queue cannot reorder or rewrite.
 */
import type {
  AppendMessage,
  ExternalThreadQueueAdapter,
  QueueItemState,
} from '@assistant-ui/react';
import debug from 'debug';
import { useCallback, useMemo } from 'react';

import { chatRemoveQueueItem } from '../../../services/chatService';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import { queueItemRemoved, type RunQueueItem } from '../../../store/queueSlice';

const log = debug('openhuman:aui-queue');

const EMPTY_ITEMS: readonly RunQueueItem[] = [];
const EMPTY_QUEUE_STATE: readonly QueueItemState[] = [];

/**
 * Projected once per store array: the composer caches its queue on array
 * identity, so a fresh array per render would re-render every queue row.
 */
const projected = new WeakMap<readonly RunQueueItem[], readonly QueueItemState[]>();

function toQueueItemStates(items: readonly RunQueueItem[]): readonly QueueItemState[] {
  if (items.length === 0) return EMPTY_QUEUE_STATE;
  let states = projected.get(items);
  if (!states) {
    states = items.map(item => ({
      id: item.id,
      prompt: item.textPreview,
      parts: [{ type: 'text' as const, text: item.textPreview }],
    }));
    projected.set(items, states);
  }
  return states;
}

export function buildOpenHumanQueueAdapter({
  items,
  send,
  remove,
}: {
  items: readonly RunQueueItem[];
  send: (message: AppendMessage) => Promise<void>;
  remove: (itemId: string) => void;
}): ExternalThreadQueueAdapter {
  const deliver = (lane: 'enqueue' | 'steer', message: AppendMessage) => {
    // The host reports its own failures (send-error banner); this only keeps a
    // rejection from going unhandled.
    send(message).catch((error: unknown) => {
      log('[aui-queue] %s send failed: %s', lane, error instanceof Error ? error.message : error);
    });
  };
  // Idle thread: the runtime calls `enqueue` exactly where it used to call
  // `onNew`, so deliver synchronously and keep that path unchanged.
  const enqueue = (message: AppendMessage) => {
    log('[aui-queue] enqueue (idle) → host send');
    queueMicrotask(() => deliver('enqueue', message));
  };
  // Running thread: the host queues it as a follow-up. One macrotask later, so
  // the composer clear the runtime made just before calling us reaches the host
  // draft first; the host restores a failed follow-up by writing the draft
  // back, and a failure landing before that clear would be wiped out by it.
  const steer = (message: AppendMessage) => {
    log('[aui-queue] steer (running) → host send, deferred');
    setTimeout(() => deliver('steer', message), 0);
  };
  return {
    items: toQueueItemStates(items),
    steerItems: EMPTY_QUEUE_STATE,
    enqueue,
    steer,
    move: queueItemId => log('[aui-queue] move ignored item=%s (core queue is fixed)', queueItemId),
    edit: queueItemId => log('[aui-queue] edit ignored item=%s (core queue is fixed)', queueItemId),
    remove,
  };
}

/** The `queue` option for `threadId`'s external store. */
export function useOpenHumanQueueAdapter(
  threadId: string | null,
  send: (message: AppendMessage) => Promise<void>
): ExternalThreadQueueAdapter {
  const dispatch = useAppDispatch();
  const items = useAppSelector(state =>
    threadId ? (state.queue?.itemsByThread[threadId] ?? EMPTY_ITEMS) : EMPTY_ITEMS
  );

  const remove = useCallback(
    (itemId: string) => {
      if (!threadId) return;
      log('[aui-queue] remove requested thread=%s item=%s', threadId, itemId);
      void chatRemoveQueueItem(threadId, itemId).then(removed => {
        if (removed) dispatch(queueItemRemoved({ threadId, itemId }));
        else log('[aui-queue] remove not confirmed thread=%s item=%s', threadId, itemId);
      });
    },
    [dispatch, threadId]
  );

  return useMemo(() => buildOpenHumanQueueAdapter({ items, send, remove }), [items, send, remove]);
}
