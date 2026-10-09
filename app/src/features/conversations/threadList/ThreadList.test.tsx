import { fireEvent, render, screen, within } from '@testing-library/react';
import { createRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Thread } from '../../../types/thread';
import { PINNED_THREAD_LABEL } from './groupThreads';
import { ThreadList } from './ThreadList';

vi.mock('../../../lib/i18n/I18nContext', () => ({ useT: () => ({ t: (key: string) => key }) }));
// The real module pulls in the whole chat surface; the list only needs the
// IME guard.
vi.mock('../Conversations', () => ({ isImeCompositionKeyEvent: () => false }));

const NOW = new Date(2026, 9, 6, 15, 0, 0);

function thread(id: string, title: string, daysAgo: number, extra: Partial<Thread> = {}): Thread {
  const at = new Date(NOW.getFullYear(), NOW.getMonth(), NOW.getDate() - daysAgo, 12);
  return {
    id,
    title,
    chatId: null,
    isActive: true,
    messageCount: 1,
    lastMessageAt: at.toISOString(),
    createdAt: at.toISOString(),
    labels: [],
    ...extra,
  };
}

const THREADS = [
  thread('t1', 'Fix Gmail OAuth', 0),
  thread('t2', 'Plan trip', 1),
  thread('t3', 'Old notes', 60, { labels: [PINNED_THREAD_LABEL] }),
  thread('t4', 'Refactor sidebar', 3, { actionDir: '/home/me/projects/site/' }),
];

function renderList(props: Partial<Parameters<typeof ThreadList>[0]> = {}) {
  const titles = new Map(THREADS.map(t => [t.id, t.title]));
  return render(
    <ThreadList
      threads={THREADS}
      selectedThreadId={null}
      onCreateThread={vi.fn()}
      onSelectThread={vi.fn()}
      resolveTitle={id => titles.get(id) ?? id}
      onRequestDelete={vi.fn()}
      editingThreadId={null}
      editTitleValue=""
      editTitleInputRef={createRef<HTMLInputElement>()}
      onEditTitleValueChange={vi.fn()}
      onStartEditTitle={vi.fn()}
      onCommitTitle={vi.fn()}
      onCancelEditTitle={vi.fn()}
      onBlurTitle={vi.fn()}
      {...props}
    />
  );
}

describe('ThreadList', () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(NOW);
  });
  afterEach(() => vi.useRealTimers());

  it('groups threads into pinned and recency sections in order', () => {
    renderList();
    const sections = screen.getAllByRole('region');
    expect(sections.map(s => s.getAttribute('data-testid'))).toEqual([
      'thread-group-pinned',
      'thread-group-today',
      'thread-group-yesterday',
      'thread-group-previous7Days',
    ]);
    expect(within(sections[0]).getByTestId('thread-row-t3')).toBeInTheDocument();
  });

  it('has no search box of its own (search lives in the global palette)', () => {
    renderList();
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();
  });

  it('toggles a pin without selecting the row', () => {
    const onTogglePin = vi.fn();
    const onSelectThread = vi.fn();
    renderList({ onTogglePin, onSelectThread });
    fireEvent.click(screen.getByTestId('thread-pin-t1'));
    expect(onTogglePin).toHaveBeenCalledWith(expect.objectContaining({ id: 't1' }), true);
    fireEvent.click(screen.getByTestId('thread-pin-t3'));
    expect(onTogglePin).toHaveBeenCalledWith(expect.objectContaining({ id: 't3' }), false);
    expect(onSelectThread).not.toHaveBeenCalled();
  });

  it('hides the pin action when no handler is given', () => {
    renderList();
    expect(screen.queryByTestId('thread-pin-t1')).not.toBeInTheDocument();
  });

  it('shows unread only for idle threads and shimmers running ones', () => {
    renderList({ unreadThreadIds: new Set(['t1', 't2']), isThreadRunning: id => id === 't2' });
    expect(screen.getByTestId('thread-unread-t1')).toBeInTheDocument();
    expect(screen.queryByTestId('thread-unread-t2')).not.toBeInTheDocument();
    const running = within(screen.getByTestId('thread-row-t2')).getByText('Plan trip');
    expect(running).toHaveAttribute('data-running', 'true');
    expect(running.className).toContain('shimmer');
  });

  it('puts the working folder name in the row tooltip', () => {
    renderList();
    expect(screen.getByTestId('thread-row-t4')).toHaveAttribute(
      'title',
      'chat.sidebar.workingFolder'.replace('{folder}', 'site')
    );
    expect(screen.getByTestId('thread-row-t1')).not.toHaveAttribute('title');
  });
});
