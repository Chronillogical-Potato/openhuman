import type { ToolCallMessagePartComponent } from '@assistant-ui/react';

import { TodoList, type TodoItem, type TodoStatus } from '../../../components/assistant-ui/elements/todo-list';
import { useT } from '../../../lib/i18n/I18nContext';
import type { CoreTodoStatus } from '../../../store/threadTodosSlice';

/**
 * Adapts the core wire shape of a `todo` tool call — `{content, status}` with
 * `status: 'pending'|'in_progress'|'completed'` — onto the vendored
 * `TodoList` element's `TodoItem` shape (`{id, text, status}` with
 * `status: 'pending'|'active'|'done'|'failed'`). The core never sends
 * `'failed'` today, so that status never maps — kept here anyway so the
 * mapping stays honest if the core ever adds it.
 */
export function mapCoreTodoStatus(status: unknown): TodoStatus {
  switch (status) {
    case 'in_progress':
      return 'active';
    case 'completed':
      return 'done';
    case 'pending':
      return 'pending';
    default:
      return 'pending';
  }
}

interface CoreTodoItem {
  content?: unknown;
  status?: unknown;
}

function isCoreTodoItem(value: unknown): value is CoreTodoItem {
  return Boolean(value) && typeof value === 'object';
}

/** `content`+index as a stable id when the payload has no id field of its own. */
export function toAuiTodoItems(raw: unknown): TodoItem[] {
  if (!Array.isArray(raw)) return [];
  const items: TodoItem[] = [];
  raw.forEach((candidate, index) => {
    if (!isCoreTodoItem(candidate)) return;
    const content = typeof candidate.content === 'string' ? candidate.content : '';
    if (!content.trim()) return;
    items.push({
      id: `${index}-${content}`,
      text: content,
      status: mapCoreTodoStatus(candidate.status),
    });
  });
  return items;
}

/** Args/result shape the core sends for the `todo` tool call. */
interface TodoToolArgs {
  todos?: Array<{ content: string; status: CoreTodoStatus }>;
}

/**
 * Toolkit render for the `todo` tool call — a standalone element in the
 * transcript showing the todo list snapshot as of THIS call (args carry the
 * write while in flight; result echoes it back once settled). The pinned,
 * always-current list above the composer is a separate render, driven by
 * {@link useThreadTodos} off the live `thread_todos_changed` event, not this
 * per-call snapshot.
 */
export const TodoListPart: ToolCallMessagePartComponent = ({ args, result }) => {
  const { t } = useT();
  const payload = (result ?? args) as TodoToolArgs | undefined;
  const items = toAuiTodoItems(payload?.todos);
  if (items.length === 0) return null;
  return <TodoList items={items} title={t('conversations.todos.title')} />;
};
