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

/** One turn's rows. The selectors take turns oldest-first. */
const turn = (...entries: ToolTimelineEntry[]) => entries;

const todoResult = (todos: Array<{ content: string; status?: string }>) => ({
  sessionId: 's1',
  todos,
  markdown: '',
});

const goalResult = (goal: Record<string, unknown> | null) => ({ goal, text: '' });

describe('selectTodoList', () => {
  it('returns null when the agent never wrote a list', () => {
    expect(selectTodoList([])).toBeNull();
    expect(selectTodoList([turn(entry('file_read', 'contents'))])).toBeNull();
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
    // Delivered out of order within the turn: the later write landed first in
    // the array, but its `seq` is higher.
    const list = selectTodoList([turn(second, first)]);
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

  it('prefers the newest turn: a later turn supersedes an earlier list', () => {
    const earlier = turn(entry('todo', todoResult([{ content: 'Plan', status: 'in_progress' }])));
    const later = turn(entry('todo', todoResult([{ content: 'Plan', status: 'completed' }])));
    expect(selectTodoList([earlier, later])?.items[0].status).toBe('completed');
  });

  it('falls back to an earlier turn when the newest turn wrote no list', () => {
    const wrote = turn(entry('todo', todoResult([{ content: 'Plan', status: 'in_progress' }])));
    const quiet = turn(entry('file_read', 'contents'));
    expect(selectTodoList([wrote, quiet])?.items[0].content).toBe('Plan');
  });

  it('skips failed, running, and unparseable todo rows', () => {
    const good = entry('todo', todoResult([{ content: 'Only this', status: 'pending' }]));
    const failed = entry('todo', 'only one todo may be in_progress', { status: 'error' });
    const running = entry('todo', undefined, { status: 'running', result: undefined });
    const garbage = entry('todo', 'not json');
    const list = selectTodoList([turn(good, failed, running, garbage)]);
    expect(list?.items.map(i => i.content)).toEqual(['Only this']);
  });

  it('treats an empty write as a cleared list', () => {
    const wrote = entry('todo', todoResult([{ content: 'x', status: 'pending' }]));
    const cleared = entry('todo', todoResult([]));
    expect(selectTodoList([turn(wrote, cleared)])).toBeNull();
  });

  it('defaults an unknown status to pending and drops blank content', () => {
    const list = selectTodoList([
      turn(
        entry(
          'todo',
          todoResult([
            { content: '  spaced  ', status: 'blocked' },
            { content: '   ' },
            { content: 'done', status: 'completed' },
          ])
        )
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
      turn(
        entry(
          'todo',
          todoResult([
            { content: 'a', status: 'completed' },
            { content: 'b', status: 'completed' },
          ])
        )
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
    expect(selectThreadGoal([turn(entry('todo', todoResult([])))])).toBeNull();
  });

  it('reads the goal a goal_set wrote', () => {
    expect(selectThreadGoal([turn(entry('goal_set', goalResult(active)))])).toEqual({
      goalId: 'g1',
      objective: 'Ship the release',
      status: 'active',
      tokensUsed: 1200,
      tokenBudget: 50000,
    });
  });

  // The reason the selectors scan every turn: a goal is set in the turn the
  // work starts in and then goes untouched for turns on end.
  it('keeps a goal set several turns ago', () => {
    const set = turn(entry('goal_set', goalResult(active)));
    const working = turn(entry('todo', todoResult([{ content: 'Plan', status: 'in_progress' }])));
    const stillWorking = turn(entry('file_read', 'contents'));
    expect(selectThreadGoal([set, working, stillWorking])?.goalId).toBe('g1');
  });

  it('follows the newest call: goal_complete supersedes goal_set', () => {
    const set = turn(entry('goal_set', goalResult(active)));
    const done = turn(entry('goal_complete', goalResult({ ...active, status: 'complete' })));
    expect(selectThreadGoal([set, done])?.status).toBe('complete');
  });

  it('clears the banner when goal_get reports no goal', () => {
    const set = turn(entry('goal_set', goalResult(active)));
    const absent = turn(entry('goal_get', goalResult(null)));
    expect(selectThreadGoal([set, absent])).toBeNull();
  });

  it('ignores errored calls and payloads without a goal field', () => {
    const set = entry('goal_set', goalResult(active));
    const failed = entry('goal_set', 'Missing objective', { status: 'error' });
    const other = entry('goal_get', { text: 'legacy text-only shape' });
    expect(selectThreadGoal([turn(set, failed, other)])?.goalId).toBe('g1');
  });

  // `goal_set` / `goal_get` sit in the `goals` tool pack, so the model calls
  // them through `use_skill` and the row is named for the wrapper.
  it('reads a goal call made through the use_skill wrapper', () => {
    expect(selectThreadGoal([turn(entry('use_skill', goalResult(active)))])?.goalId).toBe('g1');
  });

  it('ignores an unrelated use_skill result', () => {
    const set = entry('goal_set', goalResult(active));
    const unrelated = entry('use_skill', { ok: true, goal: 'a bare string, not a goal' });
    expect(selectThreadGoal([turn(set, unrelated)])?.goalId).toBe('g1');
  });

  it('treats a missing budget as unbounded', () => {
    const goal = selectThreadGoal([
      turn(
        entry('goal_set', goalResult({ ...active, tokenBudget: undefined, tokensUsed: undefined }))
      ),
    ]);
    expect(goal?.tokenBudget).toBeNull();
    expect(goal?.tokensUsed).toBe(0);
  });
});
