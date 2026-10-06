import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { Hit } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import MemoryLearningsTab from './MemoryLearningsTab';

const hoisted = vi.hoisted(() => ({
  list: vi.fn(),
  learn: vi.fn(),
  forget: vi.fn(),
  jobs: vi.fn(),
}));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryItemsList: (...a: unknown[]) => hoisted.list(...a),
  memoryLearn: (...a: unknown[]) => hoisted.learn(...a),
  memoryForget: (...a: unknown[]) => hoisted.forget(...a),
  memoryJobsList: (...a: unknown[]) => hoisted.jobs(...a),
}));

function pendingBuild(id: string) {
  return {
    id,
    root: 'default',
    job: { job: 'build_beliefs' },
    queued_at: '2026-10-06T00:00:00Z',
    attempts: 0,
  };
}

function learning(id: string, text: string): Hit {
  return { id, kind: 'learning', text, meta: {}, score: 0 };
}

beforeEach(() => {
  hoisted.list.mockReset();
  hoisted.learn.mockReset();
  hoisted.forget.mockReset();
  hoisted.jobs.mockReset();
  hoisted.jobs.mockResolvedValue({ pending: [], history: [] });
});

describe('MemoryLearningsTab', () => {
  it('lists learnings filtered to the learning kind', async () => {
    hoisted.list.mockResolvedValue({ items: [learning('l1', 'Prefers dark mode')] });
    renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learning-l1')).toHaveTextContent('Prefers dark mode');
    expect(hoisted.list).toHaveBeenCalledWith({
      filter: { kinds: ['learning'] },
      limit: 20,
      cursor: undefined,
    });
  });

  it('badges learnings the belief builder derived', async () => {
    hoisted.list.mockResolvedValue({
      items: [
        { ...learning('b1', 'Works best in the morning'), meta: { tags: ['belief'] } },
        { ...learning('l1', 'Prefers dark mode'), meta: { tags: ['ui'] } },
      ],
    });
    renderWithProviders(<MemoryLearningsTab />);
    const belief = await screen.findByTestId('memory-learning-b1');
    expect(within(belief).getByTestId('memory-hit-belief')).toHaveTextContent('Built belief');
    expect(
      within(screen.getByTestId('memory-learning-l1')).queryByTestId('memory-hit-belief')
    ).not.toBeInTheDocument();
  });

  it('shows the empty state', async () => {
    hoisted.list.mockResolvedValue({ items: [] });
    renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learnings-empty')).toBeInTheDocument();
    await waitFor(() => expect(hoisted.jobs).toHaveBeenCalled());
    expect(screen.queryByTestId('memory-learnings-deriving')).not.toBeInTheDocument();
  });

  it('says beliefs are still building when the list is empty and a build is pending', async () => {
    hoisted.list.mockResolvedValue({ items: [] });
    hoisted.jobs.mockResolvedValue({
      pending: [pendingBuild('j1'), { ...pendingBuild('j2'), job: { job: 'ingest_brain' } }],
      history: [],
    });
    renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learnings-deriving')).toHaveTextContent(
      'still building beliefs'
    );
    expect(screen.queryByTestId('memory-learnings-empty')).not.toBeInTheDocument();
  });

  it('keeps the plain empty state when only other jobs are pending or the queue cannot be read', async () => {
    hoisted.list.mockResolvedValue({ items: [] });
    hoisted.jobs.mockResolvedValue({
      pending: [{ ...pendingBuild('j2'), job: { job: 'ingest_brain' } }],
      history: [],
    });
    const { unmount } = renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learnings-empty')).toBeInTheDocument();
    await waitFor(() => expect(hoisted.jobs).toHaveBeenCalled());
    expect(screen.queryByTestId('memory-learnings-deriving')).not.toBeInTheDocument();
    unmount();

    hoisted.jobs.mockRejectedValue(new Error('core offline'));
    renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learnings-empty')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-learnings-deriving')).not.toBeInTheDocument();
  });

  it('does not read the job queue when there are learnings', async () => {
    hoisted.list.mockResolvedValue({ items: [learning('l1', 'Prefers dark mode')] });
    renderWithProviders(<MemoryLearningsTab />);
    await screen.findByTestId('memory-learning-l1');
    expect(hoisted.jobs).not.toHaveBeenCalled();
  });

  it('pages with the cursor', async () => {
    hoisted.list
      .mockResolvedValueOnce({ items: [learning('l1', 'one')], next_cursor: 'c2' })
      .mockResolvedValueOnce({ items: [learning('l2', 'two')] });
    renderWithProviders(<MemoryLearningsTab />);
    fireEvent.click(await screen.findByTestId('memory-learnings-more'));
    expect(await screen.findByTestId('memory-learning-l2')).toBeInTheDocument();
    expect(screen.getByTestId('memory-learning-l1')).toBeInTheDocument();
    expect(hoisted.list).toHaveBeenLastCalledWith({
      filter: { kinds: ['learning'] },
      limit: 20,
      cursor: 'c2',
    });
    expect(screen.queryByTestId('memory-learnings-more')).not.toBeInTheDocument();
  });

  it('adds a learning with the chosen kind and reloads', async () => {
    hoisted.list
      .mockResolvedValueOnce({ items: [] })
      .mockResolvedValueOnce({ items: [learning('l9', 'Always use metric units')] });
    hoisted.learn.mockResolvedValue({ id: 'l9' });
    renderWithProviders(<MemoryLearningsTab />);
    await screen.findByTestId('memory-learnings-empty');

    fireEvent.change(screen.getByTestId('memory-learning-input'), {
      target: { value: 'Always use metric units' },
    });
    fireEvent.change(screen.getByTestId('memory-learning-kind'), {
      target: { value: 'preference' },
    });
    fireEvent.click(screen.getByTestId('memory-learning-add'));

    await waitFor(() =>
      expect(hoisted.learn).toHaveBeenCalledWith({
        text: 'Always use metric units',
        kind: 'preference',
      })
    );
    expect(await screen.findByTestId('memory-learning-l9')).toBeInTheDocument();
    expect(screen.getByTestId('memory-learning-input')).toHaveValue('');
  });

  it('deletes a learning', async () => {
    hoisted.list.mockResolvedValue({ items: [learning('l1', 'old')] });
    hoisted.forget.mockResolvedValue({ forgotten: 1 });
    renderWithProviders(<MemoryLearningsTab />);
    fireEvent.click(await screen.findByTestId('memory-learning-delete-l1'));
    await waitFor(() => expect(hoisted.forget).toHaveBeenCalledWith(['l1']));
    await waitFor(() => expect(screen.queryByTestId('memory-learning-l1')).not.toBeInTheDocument());
  });

  it('shows a load error', async () => {
    hoisted.list.mockRejectedValue(new Error('MEMORY_OFF: no engine'));
    renderWithProviders(<MemoryLearningsTab />);
    expect(await screen.findByTestId('memory-learnings-error')).toHaveTextContent('no engine');
  });
});
