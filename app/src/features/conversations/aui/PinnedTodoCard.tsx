/**
 * The thread's live todo list pinned above the composer, as OpenClaw's
 * collapsible session progress card (`TodoProgressCard`).
 *
 * Open by default while a step is in progress and collapsed otherwise, until
 * the user opens or closes it by hand: that choice is remembered per thread in
 * `userScopedStorage` and wins from then on.
 */
import debugFactory from 'debug';
import { useCallback, useEffect, useState } from 'react';

import {
  type TodoItem,
  TodoProgressCard,
  todoProgress,
} from '../../../components/assistant-ui/elements/todo-list';
import { useT } from '../../../lib/i18n/I18nContext';
import { userScopedStorage } from '../../../store/userScopedStorage';

const debug = debugFactory('conversations:todos');

/** `userScopedStorage` key holding `{ [threadId]: open }`. */
export const TODO_CARD_OPEN_KEY = 'chat.todoCardOpen';
/** How many threads' choices are kept; the oldest are dropped first. */
const MAX_REMEMBERED = 50;

type OpenMap = Record<string, boolean>;

async function readOpenMap(): Promise<OpenMap> {
  try {
    const raw = await userScopedStorage.getItem(TODO_CARD_OPEN_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === 'object' ? (parsed as OpenMap) : {};
  } catch {
    return {};
  }
}

async function rememberOpen(threadId: string, open: boolean): Promise<void> {
  const map = await readOpenMap();
  delete map[threadId];
  map[threadId] = open;
  const keys = Object.keys(map);
  for (const stale of keys.slice(0, Math.max(0, keys.length - MAX_REMEMBERED))) {
    delete map[stale];
  }
  await userScopedStorage.setItem(TODO_CARD_OPEN_KEY, JSON.stringify(map));
}

/** The card's open state: the user's remembered choice, else open while a step runs. */
export function useTodoCardOpen(
  threadId: string,
  anyActive: boolean
): [boolean, (open: boolean) => void] {
  const [remembered, setRemembered] = useState<{ threadId: string; open: boolean } | null>(null);

  useEffect(() => {
    let cancelled = false;
    void readOpenMap().then(map => {
      if (cancelled) return;
      const open = map[threadId];
      setRemembered(typeof open === 'boolean' ? { threadId, open } : null);
    });
    return () => {
      cancelled = true;
    };
  }, [threadId]);

  const setOpen = useCallback(
    (open: boolean) => {
      setRemembered({ threadId, open });
      debug('todo card %s thread=%s', open ? 'opened' : 'closed', threadId);
      void rememberOpen(threadId, open);
    },
    [threadId]
  );

  const choice = remembered?.threadId === threadId ? remembered.open : undefined;
  return [choice ?? anyActive, setOpen];
}

export function PinnedTodoCard({
  threadId,
  items,
  className,
}: {
  threadId: string;
  items: readonly TodoItem[];
  className?: string;
}) {
  const { t } = useT();
  const progress = todoProgress(items);
  const [open, setOpen] = useTodoCardOpen(threadId, progress.anyActive);
  return (
    <TodoProgressCard
      data-testid="todo-checklist"
      items={items}
      open={open}
      onOpenChange={setOpen}
      title={t('conversations.todos.title')}
      completedLabel={t('chat.todos.completed')}
      countLabel={t('chat.todos.ofTotal')
        .replace('{done}', String(progress.done))
        .replace('{total}', String(progress.total))}
      className={className}
    />
  );
}

export default PinnedTodoCard;
