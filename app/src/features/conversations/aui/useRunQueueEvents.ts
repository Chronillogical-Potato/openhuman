/**
 * Mirror the core's `queue_item_*` socket events into `queueSlice`, which the
 * composer's `queue` adapter reads. Mounted once, by `ChatRuntimeProvider`,
 * while the socket is connected: events are keyed by thread, so every thread's
 * queue stays current whichever one is on screen.
 */
import { useEffect } from 'react';

import { subscribeQueueEvents } from '../../../services/chatService';
import { useAppDispatch } from '../../../store/hooks';
import { queueItemDelivered, queueItemQueued, queueItemRemoved } from '../../../store/queueSlice';

export function useRunQueueEvents(enabled: boolean): void {
  const dispatch = useAppDispatch();

  useEffect(() => {
    if (!enabled) return;
    return subscribeQueueEvents({
      onQueued: e => dispatch(queueItemQueued({ threadId: e.thread_id, item: e.queue_item })),
      onDelivered: e =>
        dispatch(queueItemDelivered({ threadId: e.thread_id, itemId: e.queue_item.id })),
      onRemoved: e =>
        dispatch(queueItemRemoved({ threadId: e.thread_id, itemId: e.queue_item.id })),
    });
  }, [dispatch, enabled]);
}
