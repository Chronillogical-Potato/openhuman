'use client';

/**
 * A checklist of the agent's steps for the current thread, ticking off as it
 * works.
 *
 * Vendored from the assistant-ui `elements-todo-list` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-todo-list.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 * - The header text `"Todos"` is a prop (`title`) with that English default,
 *   so the caller (`TodoListPart` in
 *   `features/conversations/aui/TodoListPart.tsx`) supplies the translated
 *   string via `useT()`.
 * - OpenHuman additions after OpenClaw's session progress card:
 *   {@link todoProgress}, the collapsible {@link TodoProgressCard} pinned above
 *   the composer, and the one-line {@link TodoReceipt} a `todo` tool call
 *   leaves in the transcript. Every string they show is a prop.
 */
import { cn } from '@/components/assistant-ui/lib/utils';
import { CheckIcon, ChevronRightIcon, ListChecksIcon, Loader2Icon, XIcon } from 'lucide-react';
import type { ComponentProps, ReactNode } from 'react';

import { mono } from './surfaces';

export type TodoStatus = 'pending' | 'active' | 'done' | 'failed';

export interface TodoItem {
  id: string;
  text: string;
  status: TodoStatus;
  reason?: string;
}

export function TodoList({
  items,
  revision,
  title = 'Todos',
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'children' | 'items' | 'revision' | 'title'> & {
  items: readonly TodoItem[];
  revision?: number;
  title?: string;
}) {
  const done = items.filter(item => item.status === 'done').length;

  return (
    <div
      data-slot="todo-list"
      className={cn('flex w-full max-w-sm flex-col gap-3', className)}
      {...props}>
      <div className="flex items-baseline justify-between">
        <span className="text-[13.5px] font-medium">{title}</span>
        <span className={cn(mono, 'text-foreground/35 tabular-nums')}>
          {revision === undefined
            ? `${done}/${items.length}`
            : `${done}/${items.length} · rev ${revision}`}
        </span>
      </div>
      <TodoItems items={items} />
    </div>
  );
}

/** The step rows, shared by every todo surface. */
export function TodoItems({ items }: { items: readonly TodoItem[] }) {
  return (
    <ul className="flex flex-col gap-1">
      {items.map(item => (
        <li
          key={item.id}
          data-testid="todo-item"
          data-status={item.status}
          className="fade-in slide-in-from-bottom-1 animate-in fill-mode-both flex items-start gap-2.5 py-0.5 text-[13.5px] duration-300">
          <span aria-hidden className="flex size-4 h-5 shrink-0 items-center justify-center">
            {item.status === 'done' ? (
              <span className="border-foreground/20 bg-foreground/[0.06] flex size-3.5 items-center justify-center rounded-[5px] border">
                <CheckIcon className="text-foreground/45 size-2.5" />
              </span>
            ) : item.status === 'failed' ? (
              <span className="flex size-3.5 items-center justify-center rounded-[5px] border border-red-600/25 bg-red-600/[0.08] dark:border-red-400/25 dark:bg-red-400/[0.08]">
                <XIcon className="size-2.5 text-red-600 dark:text-red-400" />
              </span>
            ) : item.status === 'active' ? (
              <Loader2Icon className="size-3.5 animate-spin text-blue-500 motion-reduce:animate-none dark:text-blue-400" />
            ) : (
              <span className="border-foreground/15 size-3.5 rounded-[5px] border" />
            )}
          </span>
          <span className="sr-only">{item.status}</span>
          <div className="min-w-0 flex-1 leading-5 break-words">
            <span
              className={cn(
                item.status === 'done' && 'text-foreground/35 line-through decoration-[1.5px]',
                item.status === 'active' && 'text-foreground/90',
                item.status === 'pending' && 'text-foreground/50',
                item.status === 'failed' && 'text-red-600 dark:text-red-400'
              )}>
              {item.text}
            </span>
            {item.status === 'failed' && item.reason ? (
              <p className="text-foreground/45 text-xs leading-4 break-words">{item.reason}</p>
            ) : null}
          </div>
        </li>
      ))}
    </ul>
  );
}

export interface TodoProgress {
  done: number;
  total: number;
  /** The step to name when collapsed: first active, else first pending, else last done. */
  current: TodoItem | undefined;
  /** 1-based position of {@link TodoProgress.current}, or 0 when there is none. */
  position: number;
  allDone: boolean;
  anyActive: boolean;
}

/** Counts and the "current step" a collapsed progress line shows. */
export function todoProgress(items: readonly TodoItem[]): TodoProgress {
  const done = items.filter(item => item.status === 'done').length;
  let index = items.findIndex(item => item.status === 'active');
  if (index < 0) index = items.findIndex(item => item.status === 'pending');
  if (index < 0) {
    for (let i = items.length - 1; i >= 0; i -= 1) {
      if (items[i].status === 'done') {
        index = i;
        break;
      }
    }
  }
  return {
    done,
    total: items.length,
    current: index >= 0 ? items[index] : undefined,
    position: index + 1,
    allDone: items.length > 0 && done === items.length,
    anyActive: items.some(item => item.status === 'active'),
  };
}

function ProgressMarker({ progress }: { progress: TodoProgress }) {
  if (progress.allDone) {
    return <CheckIcon aria-hidden className="size-3.5 shrink-0 text-emerald-500" />;
  }
  if (progress.anyActive) {
    return (
      <Loader2Icon
        aria-hidden
        className="size-3.5 shrink-0 animate-spin text-blue-500 motion-reduce:animate-none dark:text-blue-400"
      />
    );
  }
  return <ListChecksIcon aria-hidden className="text-foreground/40 size-3.5 shrink-0" />;
}

/**
 * The run's step list pinned above the composer, as a disclosure. Collapsed it
 * is one line — marker, the current step, `pos/total` (or the completed
 * label) — so a long plan never pushes the input off screen; expanded it
 * shows `{done} of {total}` and every step, capped in height and scrolling.
 */
export function TodoProgressCard({
  items,
  open,
  onOpenChange,
  title,
  completedLabel,
  countLabel,
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'children' | 'title'> & {
  items: readonly TodoItem[];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  /** Shown instead of `pos/total` once every step is done. */
  completedLabel: string;
  /** `{done} of {total}` in the expanded header, already interpolated. */
  countLabel: string;
}) {
  const progress = todoProgress(items);
  return (
    <div
      data-slot="todo-progress-card"
      data-open={open ? 'true' : 'false'}
      data-todo-completed={progress.done}
      data-todo-total={progress.total}
      className={cn(
        'border-border/60 bg-background/60 flex w-full max-w-md flex-col overflow-hidden rounded-xl border',
        className
      )}
      {...props}>
      <button
        type="button"
        aria-expanded={open}
        aria-label={title}
        data-analytics-id="chat-todo-progress-toggle"
        onClick={() => onOpenChange(!open)}
        className="hover:bg-foreground/[0.03] flex items-center gap-2 px-3 py-1.5 text-start transition-colors">
        <ProgressMarker progress={progress} />
        <span className="text-foreground/80 min-w-0 flex-1 truncate text-[13px]">
          {open ? title : (progress.current?.text ?? title)}
        </span>
        <span
          data-testid="todo-progress-count"
          className={cn(mono, 'text-foreground/40 shrink-0 tabular-nums')}>
          {progress.allDone
            ? completedLabel
            : open
              ? countLabel
              : `${progress.position}/${progress.total}`}
        </span>
        <ChevronRightIcon
          aria-hidden
          className={cn(
            'text-foreground/30 size-3 shrink-0 transition-transform duration-200 motion-reduce:transition-none',
            open && 'rotate-90'
          )}
        />
      </button>
      {open && (
        <div
          data-slot="todo-progress-steps"
          className="border-border/60 max-h-[min(300px,48dvh)] overflow-y-auto border-t px-3 py-2">
          <TodoItems items={items} />
        </div>
      )}
    </div>
  );
}

/**
 * What a `todo` tool call leaves in the transcript: one line ("Progress
 * updated — 3/7 · current step") that expands to that call's snapshot, so a
 * long run does not repeat the full list after every update.
 */
export function TodoReceipt({
  items,
  label,
  open,
  onOpenChange,
  extra,
}: {
  items: readonly TodoItem[];
  /** The line's text, already interpolated with the counts. */
  label: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  extra?: ReactNode;
}) {
  const progress = todoProgress(items);
  return (
    <div data-slot="todo-receipt" className="flex w-full max-w-md flex-col">
      <button
        type="button"
        aria-expanded={open}
        data-analytics-id="chat-todo-receipt-toggle"
        onClick={() => onOpenChange(!open)}
        className="text-foreground/50 hover:text-foreground/80 flex min-w-0 items-center gap-1.5 py-0.5 text-start text-[12.5px] transition-colors">
        <ListChecksIcon aria-hidden className="size-3.5 shrink-0" />
        <span className="min-w-0 truncate">
          {label}
          {progress.current && !progress.allDone ? ` · ${progress.current.text}` : ''}
        </span>
        <ChevronRightIcon
          aria-hidden
          className={cn(
            'size-3 shrink-0 transition-transform duration-200 motion-reduce:transition-none',
            open && 'rotate-90'
          )}
        />
      </button>
      {open && (
        <div className="pt-1.5 pl-5">
          <TodoItems items={items} />
        </div>
      )}
      {extra}
    </div>
  );
}
