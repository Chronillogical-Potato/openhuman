import { describe, expect, it } from 'vitest';

import type { ToolTimelineEntry } from '../../../store/chatRuntimeSlice';
import { selectThreadGoal, selectTodoList } from './harnessState';

let nextSeq = 0;

function entry(
  name: string,
  result: unknown,
  overrides: Partial<ToolTimelineEntry> = {}
): ToolTimelineEntry {
  nextSeq += 1;
  return {
    id: `${name}-${nextSeq}`,
    name,
    round: 1,
    seq: nextSeq,
    status: 'success',
    result: typeof result === 'string' ? result : JSON.stringify(result),
    ...overrides,
  };
}

const todoResult = (todos: Array<{ content: string; status?: string }>) => ({
  sessionId: 's1',
  todos,
  markdown: '',
});

const goalResult = (goal: Record<string, unknown> | null) => ({ goal, text: '' });

describe('selectTodoList', () => {
  it('returns null when the agent never wrote a list', () => {
    expect(selectTodoList([])).toBeNull();
    expect(selectTodoList([entry('file_read', 'contents')])).toBeNull();
  });

  it('reads the newest successful todo write, by issue order not array order', () => {
    const first = entry(
      'todo',
      todoResult([
        { content: 'Plan', status: 'in_progress' },
        { content: 'Build', status: 'pending' },
      ])
    );
    const second = entry(
      'todo',
      todoResult([
        { content: 'Plan', status: 'completed' },
        { content: 'Build', status: 'in_progress' },
      ])
    );
    // Delivered out of order: the later write landed first in the array.
    const list = selectTodoList([second, first]);
    expect(list).toEqual({
      items: [
        { content: 'Plan', status: 'completed' },
        { content: 'Build', status: 'in_progress' },
      ],
      completed: 1,
      total: 2,
      done: false,
    });
  });

  it('skips failed, running, and unparseable todo rows', () => {
    const good = entry('todo', todoResult([{ content: 'Only this', status: 'pending' }]));
    const failed = entry('todo', 'only one todo may be in_progress', { status: 'error' });
    const running = entry('todo', undefined, { status: 'running', result: undefined });
    const garbage = entry('todo', 'not json');
    const list = selectTodoList([good, failed, running, garbage]);
    expect(list?.items.map(i => i.content)).toEqual(['Only this']);
  });

  it('treats an empty write as a cleared list', () => {
    const wrote = entry('todo', todoResult([{ content: 'x', status: 'pending' }]));
    const cleared = entry('todo', todoResult([]));
    expect(selectTodoList([wrote, cleared])).toBeNull();
  });

  it('defaults an unknown status to pending and drops blank content', () => {
    const list = selectTodoList([
      entry(
        'todo',
        todoResult([
          { content: '  spaced  ', status: 'blocked' },
          { content: '   ' },
          { content: 'done', status: 'completed' },
        ])
      ),
    ]);
    expect(list).toEqual({
      items: [
        { content: 'spaced', status: 'pending' },
        { content: 'done', status: 'completed' },
      ],
      completed: 1,
      total: 2,
      done: false,
    });
  });

  it('reports done once every item is completed', () => {
    const list = selectTodoList([
      entry(
        'todo',
        todoResult([
          { content: 'a', status: 'completed' },
          { content: 'b', status: 'completed' },
        ])
      ),
    ]);
    expect(list?.done).toBe(true);
    expect(list?.completed).toBe(2);
  });
});

describe('selectThreadGoal', () => {
  const active = {
    threadId: 't1',
    goalId: 'g1',
    objective: 'Ship the release',
    status: 'active',
    tokenBudget: 50000,
    tokensUsed: 1200,
  };

  it('returns null without a goal call', () => {
    expect(selectThreadGoal([])).toBeNull();
    expect(selectThreadGoal([entry('todo', todoResult([]))])).toBeNull();
  });

  it('reads the goal a goal_set wrote', () => {
    expect(selectThreadGoal([entry('goal_set', goalResult(active))])).toEqual({
      goalId: 'g1',
      objective: 'Ship the release',
      status: 'active',
      tokensUsed: 1200,
      tokenBudget: 50000,
    });
  });

  it('follows the newest call: goal_complete supersedes goal_set', () => {
    const set = entry('goal_set', goalResult(active));
    const done = entry('goal_complete', goalResult({ ...active, status: 'complete' }));
    expect(selectThreadGoal([set, done])?.status).toBe('complete');
  });

  it('clears the banner when goal_get reports no goal', () => {
    const set = entry('goal_set', goalResult(active));
    const absent = entry('goal_get', goalResult(null));
    expect(selectThreadGoal([set, absent])).toBeNull();
  });

  it('ignores errored calls and payloads without a goal field', () => {
    const set = entry('goal_set', goalResult(active));
    const failed = entry('goal_set', 'Missing objective', { status: 'error' });
    const other = entry('goal_get', { text: 'legacy text-only shape' });
    expect(selectThreadGoal([set, failed, other])?.goalId).toBe('g1');
  });

  it('treats a missing budget as unbounded', () => {
    const goal = selectThreadGoal([
      entry('goal_set', goalResult({ ...active, tokenBudget: undefined, tokensUsed: undefined })),
    ]);
    expect(goal?.tokenBudget).toBeNull();
    expect(goal?.tokensUsed).toBe(0);
  });
});
