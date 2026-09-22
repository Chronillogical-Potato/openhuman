/**
 * Harness-level work state derived from a thread's tool timeline.
 *
 * The agent keeps two pieces of state while it works on a long request: a
 * **todo list** (the `todo` tool — one whole-list write per call, Claude
 * Code / Codex style) and a **thread goal** (`goal_set` / `goal_get` /
 * `goal_complete` — the durable objective for the thread). Neither has an
 * RPC of its own; the core answers each tool call with a JSON payload, and
 * that payload rides the tool result into the timeline (`ToolTimelineEntry.
 * result`) both live and on reload. So the pane shows what the agent last
 * wrote by reading the newest successful call of each kind — the same
 * mechanism the transcript uses, with no second source of truth to drift.
 *
 * Both selectors are pure so the checklist and banner can be tested without
 * a store.
 */
import type { ToolTimelineEntry } from '../../../store/chatRuntimeSlice';

export type TodoItemStatus = 'pending' | 'in_progress' | 'completed';

export interface TodoItemView {
  content: string;
  status: TodoItemStatus;
}

export interface TodoListView {
  items: TodoItemView[];
  /** Items marked `completed`. */
  completed: number;
  total: number;
  /** Whether every item is completed (and there is at least one). */
  done: boolean;
}

export type ThreadGoalStatus = 'active' | 'paused' | 'budget_limited' | 'complete';

export interface ThreadGoalView {
  goalId: string;
  objective: string;
  status: ThreadGoalStatus;
  tokensUsed: number;
  tokenBudget: number | null;
}

const TODO_TOOL = 'todo';
const GOAL_TOOLS = new Set(['goal_set', 'goal_get', 'goal_complete']);
const TODO_STATUSES: ReadonlySet<string> = new Set(['pending', 'in_progress', 'completed']);
const GOAL_STATUSES: ReadonlySet<string> = new Set([
  'active',
  'paused',
  'budget_limited',
  'complete',
]);

function parseResult(entry: ToolTimelineEntry): Record<string, unknown> | null {
  if (entry.status !== 'success' || !entry.result) return null;
  try {
    const parsed: unknown = JSON.parse(entry.result);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

/**
 * Newest-first walk of the timeline by issue order (`seq`), not array order:
 * a `tool_args_delta` for a later parallel call can land ahead of an earlier
 * one, and the last write is the one that counts.
 */
function newestFirst(timeline: ToolTimelineEntry[]): ToolTimelineEntry[] {
  return [...timeline].sort((a, b) => b.seq - a.seq);
}

function parseTodoItems(raw: unknown): TodoItemView[] | null {
  if (!Array.isArray(raw)) return null;
  const items: TodoItemView[] = [];
  for (const candidate of raw) {
    if (!candidate || typeof candidate !== 'object') continue;
    const { content, status } = candidate as { content?: unknown; status?: unknown };
    if (typeof content !== 'string' || !content.trim()) continue;
    items.push({
      content: content.trim(),
      status:
        typeof status === 'string' && TODO_STATUSES.has(status)
          ? (status as TodoItemStatus)
          : 'pending',
    });
  }
  return items;
}

/**
 * The list the agent last wrote in this thread, or `null` when it has not
 * written one (or cleared it). A sub-agent's own `todo` calls live inside its
 * parent row's `subagent.toolCalls`, never at the top level, so only the
 * thread's own agent reaches this.
 */
export function selectTodoList(timeline: ToolTimelineEntry[]): TodoListView | null {
  for (const entry of newestFirst(timeline)) {
    if (entry.name !== TODO_TOOL) continue;
    const payload = parseResult(entry);
    if (!payload) continue;
    const items = parseTodoItems(payload.todos);
    if (!items) continue;
    if (items.length === 0) return null;
    const completed = items.filter(item => item.status === 'completed').length;
    return { items, completed, total: items.length, done: completed === items.length };
  }
  return null;
}

/**
 * The thread goal as of the agent's last goal call: `goal_set` and
 * `goal_complete` carry the goal they wrote, `goal_get` the one it read (or
 * `null` when the thread has none, which clears the banner).
 */
export function selectThreadGoal(timeline: ToolTimelineEntry[]): ThreadGoalView | null {
  for (const entry of newestFirst(timeline)) {
    if (!GOAL_TOOLS.has(entry.name)) continue;
    const payload = parseResult(entry);
    if (!payload || !('goal' in payload)) continue;
    const goal = payload.goal;
    if (goal === null) return null;
    if (!goal || typeof goal !== 'object') continue;
    const { goalId, objective, status, tokensUsed, tokenBudget } = goal as Record<
      string,
      unknown
    >;
    if (typeof objective !== 'string' || typeof status !== 'string' || !GOAL_STATUSES.has(status))
      continue;
    return {
      goalId: typeof goalId === 'string' ? goalId : '',
      objective,
      status: status as ThreadGoalStatus,
      tokensUsed: typeof tokensUsed === 'number' ? tokensUsed : 0,
      tokenBudget: typeof tokenBudget === 'number' ? tokenBudget : null,
    };
  }
  return null;
}
