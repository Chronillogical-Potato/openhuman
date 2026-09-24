/**
 * The thread's live todo list, read from `threadTodosSlice` (populated by the
 * `thread_todos_changed` socket event via `ChatRuntimeProvider`, and primed
 * on thread open by {@link useLoadThreadTodos}).
 */
import { useEffect, useRef } from 'react';

import { threadApi } from '../../../services/api/threadApi';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import { setThreadTodos, type ThreadTodoItemView } from '../../../store/threadTodosSlice';

/** `null` when the thread has no live entry yet (not "empty list"). */
export function useThreadTodos(threadId: string | null): ThreadTodoItemView[] | null {
  return useAppSelector(state =>
    threadId ? (state.threadTodos.byThread[threadId] ?? null) : null
  );
}

/**
 * Loads the thread's current todo list once per `threadId` via
 * `openhuman.threads_todos_get`, so a freshly opened thread doesn't wait for
 * the next live `thread_todos_changed` event. Any failure (older core, RPC
 * error) leaves the slice untouched — the live event stream is still the
 * primary source once a turn runs.
 */
export function useLoadThreadTodos(threadId: string | null): void {
  const dispatch = useAppDispatch();
  const requestedFor = useRef<string | null>(null);

  useEffect(() => {
    if (!threadId || requestedFor.current === threadId) return;
    requestedFor.current = threadId;
    let cancelled = false;
    void (async () => {
      try {
        const todos = await threadApi.getTodos(threadId);
        if (cancelled) return;
        dispatch(setThreadTodos({ threadId, todos }));
      } catch {
        // Older core without the RPC, or a transient failure — the live
        // socket event (or the next thread open) will still populate this.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [threadId, dispatch]);
}
