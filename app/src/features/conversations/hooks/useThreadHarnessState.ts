/**
 * The thread's harness work state — its todo list and its goal — for the
 * chat pane's checklist and goal banner.
 *
 * Both are written by agent tools and answered as JSON, so the newest `todo`
 * / `goal_*` tool result in the thread *is* the state; there is no RPC of
 * their own to read and no second store to keep in sync
 * ({@link selectTodoList} / {@link selectThreadGoal} do the reading).
 *
 * The live turn's rows come from Redux, which is all the pane needs while the
 * agent is working. The settled turns are fetched once per thread from
 * `threads_turn_state_history` — the same snapshots "View processing" replays
 * — because a goal is typically set in the turn the work *started* in: without
 * the earlier turns a reopened thread would show a checklist and no goal, or
 * neither, until the agent happened to touch them again.
 */
import { useEffect, useMemo, useState } from 'react';

import { threadApi } from '../../../services/api/threadApi';
import { type ToolTimelineEntry, toolTimelineFromPersisted } from '../../../store/chatRuntimeSlice';
import {
  selectThreadGoal,
  selectTodoList,
  type ThreadGoalView,
  type TodoListView,
} from '../utils/harnessState';

const EMPTY_TURNS: ToolTimelineEntry[][] = [];

export interface ThreadHarnessState {
  todoList: TodoListView | null;
  goal: ThreadGoalView | null;
}

/**
 * Settled turns for `threadId`, oldest first. Empty until the fetch lands,
 * and on any failure: a missing history must never keep the live state off
 * the screen, and the live turn alone already covers the common case of an
 * agent working right now.
 */
function useSettledTurns(threadId: string | null): ToolTimelineEntry[][] {
  const [turns, setTurns] = useState<ToolTimelineEntry[][]>(EMPTY_TURNS);

  useEffect(() => {
    if (!threadId) {
      setTurns(EMPTY_TURNS);
      return;
    }
    // Defensive for narrow test/embedder shims that expose only a subset of
    // threadApi; production builds always provide this method.
    if (typeof threadApi.getTurnStateHistory !== 'function') {
      setTurns(EMPTY_TURNS);
      return;
    }
    let cancelled = false;
    setTurns(EMPTY_TURNS);
    void (async () => {
      try {
        // History is newest-first; the pane scans newest-last, so reverse it.
        const history = await threadApi.getTurnStateHistory(threadId);
        if (cancelled) return;
        setTurns(
          history
            .slice()
            .reverse()
            .map(turn => (turn.toolTimeline ?? []).map(toolTimelineFromPersisted))
        );
      } catch {
        if (!cancelled) setTurns(EMPTY_TURNS);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [threadId]);

  return turns;
}

/**
 * Reads the thread's todo list and goal out of `liveTimeline` (this turn) and
 * the thread's settled turns. The live turn goes last so anything the agent
 * writes right now wins over the persisted history it was restored from.
 */
export function useThreadHarnessState(
  threadId: string | null,
  liveTimeline: ToolTimelineEntry[]
): ThreadHarnessState {
  const settled = useSettledTurns(threadId);
  const turns = useMemo(() => [...settled, liveTimeline], [settled, liveTimeline]);
  return useMemo(
    () => ({ todoList: selectTodoList(turns), goal: selectThreadGoal(turns) }),
    [turns]
  );
}
