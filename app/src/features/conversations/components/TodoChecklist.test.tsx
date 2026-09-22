import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import type { TodoListView } from '../utils/harnessState';
import { TodoChecklist } from './TodoChecklist';

// Echo i18n keys so assertions read the stable key string; interpolation
// placeholders stay in the key so the count substitution is visible.
vi.mock('../../../lib/i18n/I18nContext', () => ({ useT: () => ({ t: (key: string) => key }) }));

function list(partial: Partial<TodoListView> = {}): TodoListView {
  const items = partial.items ?? [
    { content: 'Read the spec', status: 'completed' as const },
    { content: 'Write the code', status: 'in_progress' as const },
    { content: 'Run the tests', status: 'pending' as const },
  ];
  const completed = items.filter(i => i.status === 'completed').length;
  return {
    items,
    completed,
    total: items.length,
    done: items.length > 0 && completed === items.length,
    ...partial,
  };
}

describe('TodoChecklist', () => {
  it('renders every item with its status marker in list order', () => {
    render(<TodoChecklist list={list()} />);
    const rows = screen.getAllByTestId('todo-item');
    expect(rows.map(r => r.textContent)).toEqual([
      'Read the specconversations.todos.status.completed',
      'Write the codeconversations.todos.status.inProgress',
      'Run the testsconversations.todos.status.pending',
    ]);
    expect(rows.map(r => r.getAttribute('data-status'))).toEqual([
      'completed',
      'in_progress',
      'pending',
    ]);
  });

  it('shows the completed count and exposes it as data attributes', () => {
    render(<TodoChecklist list={list()} />);
    // The key carries its placeholders; the numbers are substituted in.
    expect(screen.getByTestId('todo-progress').textContent).toBe('1 of 3 done');
    const section = screen.getByTestId('todo-checklist');
    expect(section.getAttribute('data-todo-completed')).toBe('1');
    expect(section.getAttribute('data-todo-total')).toBe('3');
    expect(screen.getByTestId('todo-progress-bar').getAttribute('aria-valuenow')).toBe('33');
  });

  it('says all done once every item is completed', () => {
    render(
      <TodoChecklist
        list={list({
          items: [
            { content: 'a', status: 'completed' },
            { content: 'b', status: 'completed' },
          ],
        })}
      />
    );
    expect(screen.getByTestId('todo-progress').textContent).toBe('conversations.todos.allDone');
    expect(screen.getByTestId('todo-progress-bar').getAttribute('aria-valuenow')).toBe('100');
  });

  it('collapses to its header and expands again', () => {
    render(<TodoChecklist list={list()} />);
    const toggle = screen.getByRole('button', { expanded: true });
    fireEvent.click(toggle);
    expect(screen.queryByTestId('todo-items')).toBeNull();
    expect(screen.getByTestId('todo-progress')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { expanded: false }));
    expect(screen.getAllByTestId('todo-item')).toHaveLength(3);
  });
});
