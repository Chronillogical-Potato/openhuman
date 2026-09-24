/**
 * The thread's current goal, read from `threadGoalSlice` (populated by the
 * `thread_goal_updated` / `thread_goal_cleared` socket events via
 * `ChatRuntimeProvider`, and primed on thread open by
 * {@link useLoadThreadGoal}).
 */
import { useEffect, useRef } from 'react';

import { threadApi } from '../../../services/api/threadApi';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import { setThreadGoal, type ThreadGoalView } from '../../../store/threadGoalSlice';

/** `1234` → `1.2k`, `2500000` → `2.5M`; small counts stay exact. */
export function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
  return String(count);
}

/** `null` when the thread has no goal (or none loaded yet). */
export function useThreadGoal(threadId: string | null): ThreadGoalView | null {
  return useAppSelector(state => (threadId ? (state.threadGoal.byThread[threadId] ?? null) : null));
}

/**
 * Loads the thread's current goal once per `threadId` via
 * `openhuman.threads_goal_get`, mirroring {@link useLoadThreadTodos}.
 */
export function useLoadThreadGoal(threadId: string | null): void {
  const dispatch = useAppDispatch();
  const requestedFor = useRef<string | null>(null);

  useEffect(() => {
    if (!threadId || requestedFor.current === threadId) return;
    requestedFor.current = threadId;
    let cancelled = false;
    void (async () => {
      try {
        const goal = await threadApi.getGoal(threadId);
        if (cancelled) return;
        dispatch(setThreadGoal({ threadId, goal }));
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
