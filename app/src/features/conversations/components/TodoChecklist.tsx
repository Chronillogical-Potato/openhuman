import React, { useState } from 'react';
import { LuCheck, LuChevronDown, LuChevronUp, LuListChecks } from 'react-icons/lu';

import Progress from '../../../components/ui/Progress';
import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import type { TodoItemStatus, TodoListView } from '../utils/harnessState';

/**
 * The agent's todo list for this thread, pinned above the composer.
 *
 * Read-only progress: the agent owns the list (one whole-list `todo` write
 * per call), the pane just shows the latest write — see
 * {@link selectTodoList}. Exactly one item is `in_progress` at a time by the
 * store's invariant, so the pulse marks where the agent is; completed items
 * strike through and stay, so a five-step task reads as a checklist ticking
 * off rather than a list that shrinks. Collapses to its header so a long
 * list never crowds the composer.
 */
interface Props {
  list: TodoListView;
}

const STATUS_LABEL_KEY: Record<TodoItemStatus, string> = {
  pending: 'conversations.todos.status.pending',
  in_progress: 'conversations.todos.status.inProgress',
  completed: 'conversations.todos.status.completed',
};

const Marker: React.FC<{ status: TodoItemStatus }> = ({ status }) => {
  if (status === 'completed') {
    return (
      <span
        aria-hidden
        className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full bg-sage-500 text-white">
        <LuCheck className="h-3 w-3" strokeWidth={3} />
      </span>
    );
  }
  if (status === 'in_progress') {
    return (
      <span
        aria-hidden
        className="flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-2 border-primary-500">
        <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-primary-500" />
      </span>
    );
  }
  return (
    <span
      aria-hidden
      className="h-4 w-4 shrink-0 rounded-full border-2 border-line-strong dark:border-line-strong"
    />
  );
};

export const TodoChecklist: React.FC<Props> = ({ list }) => {
  const { t } = useT();
  const [collapsed, setCollapsed] = useState(false);
  const percent = list.total === 0 ? 0 : Math.round((list.completed / list.total) * 100);
  const progressLabel = t('conversations.todos.progress')
    .replace('{completed}', String(list.completed))
    .replace('{total}', String(list.total));

  return (
    <section
      aria-label={t('conversations.todos.title')}
      data-testid="todo-checklist"
      data-todo-completed={list.completed}
      data-todo-total={list.total}
      className={cn(
        'mb-2 rounded-xl border bg-surface p-3 text-sm shadow-sm',
        list.done
          ? 'border-sage-200 dark:border-sage-500/30'
          : 'border-line dark:border-line-strong'
      )}>
      <button
        type="button"
        data-analytics-id="todo-checklist-toggle"
        aria-expanded={!collapsed}
        onClick={() => setCollapsed(prev => !prev)}
        className="flex w-full items-center gap-2 text-left">
        <LuListChecks
          aria-hidden
          className="h-4 w-4 shrink-0 text-primary-700 dark:text-primary-200"
        />
        <span className="font-semibold text-content-primary">
          {t('conversations.todos.title')}
        </span>
        <span className="ml-auto text-xs text-content-secondary" data-testid="todo-progress">
          {list.done ? t('conversations.todos.allDone') : progressLabel}
        </span>
        {collapsed ? (
          <LuChevronDown aria-hidden className="h-4 w-4 shrink-0 text-content-faint" />
        ) : (
          <LuChevronUp aria-hidden className="h-4 w-4 shrink-0 text-content-faint" />
        )}
      </button>

      <Progress
        value={percent}
        aria-label={progressLabel}
        data-testid="todo-progress-bar"
        className="mt-2"
      />

      {!collapsed && (
        <ol className="mt-2 max-h-56 space-y-1.5 overflow-y-auto" data-testid="todo-items">
          {list.items.map((item, i) => (
            <li
              key={`${i}-${item.content}`}
              data-testid="todo-item"
              data-status={item.status}
              className="flex items-start gap-2">
              <span className="mt-0.5">
                <Marker status={item.status} />
              </span>
              <span
                className={cn(
                  'min-w-0 flex-1 wrap-break-word',
                  item.status === 'completed' && 'text-content-faint line-through',
                  item.status === 'in_progress' && 'font-medium text-content-primary',
                  item.status === 'pending' && 'text-content-secondary'
                )}>
                {item.content}
              </span>
              <span className="sr-only">{t(STATUS_LABEL_KEY[item.status])}</span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
};

export default TodoChecklist;
