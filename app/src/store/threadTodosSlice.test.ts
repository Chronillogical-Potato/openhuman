import { describe, expect, it } from 'vitest';

import reducer, {
  clearThreadTodos,
  setThreadTodos,
  type ThreadTodosState,
} from './threadTodosSlice';

const initial: ThreadTodosState = { byThread: {} };

describe('threadTodosSlice', () => {
  it('sets the todo list for a thread', () => {
    const todos = [{ content: 'Write tests', status: 'pending' as const }];
    const next = reducer(initial, setThreadTodos({ threadId: 't1', todos }));
    expect(next.byThread.t1).toEqual(todos);
  });

  it('overwrites the previous list for the same thread', () => {
    const first = reducer(
      initial,
      setThreadTodos({ threadId: 't1', todos: [{ content: 'A', status: 'pending' }] })
    );
    const second = reducer(
      first,
      setThreadTodos({ threadId: 't1', todos: [{ content: 'B', status: 'completed' }] })
    );
    expect(second.byThread.t1).toEqual([{ content: 'B', status: 'completed' }]);
  });

  it('keeps other threads untouched', () => {
    const withT1 = reducer(
      initial,
      setThreadTodos({ threadId: 't1', todos: [{ content: 'A', status: 'pending' }] })
    );
    const withT2 = reducer(
      withT1,
      setThreadTodos({ threadId: 't2', todos: [{ content: 'B', status: 'pending' }] })
    );
    expect(withT2.byThread.t1).toHaveLength(1);
    expect(withT2.byThread.t2).toHaveLength(1);
  });

  it('clears a thread', () => {
    const withT1 = reducer(
      initial,
      setThreadTodos({ threadId: 't1', todos: [{ content: 'A', status: 'pending' }] })
    );
    const cleared = reducer(withT1, clearThreadTodos({ threadId: 't1' }));
    expect(cleared.byThread.t1).toBeUndefined();
  });
});
