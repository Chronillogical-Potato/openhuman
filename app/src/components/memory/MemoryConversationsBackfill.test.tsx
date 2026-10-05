import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { BackfillView } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import MemoryConversationsBackfill, { BACKFILL_POLL_MS } from './MemoryConversationsBackfill';

const hoisted = vi.hoisted(() => ({ status: vi.fn(), start: vi.fn() }));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryConversationsBackfillStatus: (...a: unknown[]) => hoisted.status(...a),
  memoryConversationsBackfillStart: (...a: unknown[]) => hoisted.start(...a),
}));

function view(
  phase: BackfillView['state']['phase'],
  pending: [number, number],
  extra: Partial<BackfillView['state']> = {}
): BackfillView {
  return {
    state: { phase, threads_total: 0, threads_done: 0, turns_stored: 0, items_stored: 0, ...extra },
    pending_threads: pending[0],
    pending_turns: pending[1],
  };
}

beforeEach(() => {
  hoisted.status.mockReset();
  hoisted.start.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('MemoryConversationsBackfill', () => {
  it('says how many past chats are unsynced and syncs them after consent', async () => {
    hoisted.status.mockResolvedValue(view('idle', [3, 12]));
    hoisted.start.mockResolvedValue(view('running', [3, 12], { threads_total: 3 }));
    renderWithProviders(<MemoryConversationsBackfill />);

    expect(await screen.findByTestId('memory-backfill-pending')).toHaveTextContent('3 chats');
    expect(screen.getByTestId('memory-backfill-pending')).toHaveTextContent('12 turns');

    fireEvent.click(screen.getByTestId('memory-backfill-open'));
    expect(await screen.findByTestId('memory-backfill-consent')).toBeInTheDocument();
    expect(hoisted.start).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('memory-backfill-confirm'));

    await waitFor(() => expect(hoisted.start).toHaveBeenCalledTimes(1));
    expect(await screen.findByTestId('memory-backfill-running')).toBeInTheDocument();
    expect(screen.getByTestId('memory-backfill-open')).toBeDisabled();
  });

  it('cancelling the consent uploads nothing', async () => {
    hoisted.status.mockResolvedValue(view('idle', [1, 2]));
    renderWithProviders(<MemoryConversationsBackfill />);
    fireEvent.click(await screen.findByTestId('memory-backfill-open'));
    fireEvent.click(await screen.findByTestId('memory-backfill-cancel'));
    await waitFor(() =>
      expect(screen.queryByTestId('memory-backfill-consent')).not.toBeInTheDocument()
    );
    expect(hoisted.start).not.toHaveBeenCalled();
  });

  it('is disabled when everything is synced and reports the last run', async () => {
    hoisted.status.mockResolvedValue(
      view('done', [0, 0], { threads_done: 4, turns_stored: 9, threads_total: 4 })
    );
    renderWithProviders(<MemoryConversationsBackfill />);
    expect(await screen.findByTestId('memory-backfill-done')).toHaveTextContent('9');
    expect(screen.getByTestId('memory-backfill-pending')).toHaveTextContent(
      'All past conversations are in memory.'
    );
    expect(screen.getByTestId('memory-backfill-open')).toBeDisabled();
  });

  it('polls a running sync until it finishes', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    hoisted.status
      .mockResolvedValueOnce(view('running', [2, 5], { threads_total: 2, threads_done: 1 }))
      .mockResolvedValue(
        view('done', [0, 0], { threads_total: 2, threads_done: 2, turns_stored: 5 })
      );
    renderWithProviders(<MemoryConversationsBackfill />);
    expect(await screen.findByTestId('memory-backfill-running')).toHaveTextContent('1');
    await act(async () => {
      await vi.advanceTimersByTimeAsync(BACKFILL_POLL_MS + 10);
    });
    expect(await screen.findByTestId('memory-backfill-done')).toBeInTheDocument();
  });

  it('offers to resume a failed sync and shows why it stopped', async () => {
    hoisted.status.mockResolvedValue(view('error', [1, 3], { error: 'engine unreachable' }));
    renderWithProviders(<MemoryConversationsBackfill />);
    expect(await screen.findByTestId('memory-backfill-failed')).toHaveTextContent(
      'engine unreachable'
    );
    expect(screen.getByTestId('memory-backfill-open')).toHaveTextContent('Resume sync');
  });

  it('shows status and start failures', async () => {
    hoisted.status.mockRejectedValueOnce(new Error('status broke'));
    renderWithProviders(<MemoryConversationsBackfill />);
    expect(await screen.findByTestId('memory-backfill-error')).toHaveTextContent('status broke');
  });

  it('shows a refused start', async () => {
    hoisted.status.mockResolvedValue(view('idle', [1, 1]));
    hoisted.start.mockRejectedValue(new Error('memory is off'));
    renderWithProviders(<MemoryConversationsBackfill />);
    fireEvent.click(await screen.findByTestId('memory-backfill-open'));
    fireEvent.click(await screen.findByTestId('memory-backfill-confirm'));
    expect(await screen.findByTestId('memory-backfill-error')).toHaveTextContent('memory is off');
  });
});
