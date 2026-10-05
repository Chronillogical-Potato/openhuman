import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { JobsList } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import MemoryBackgroundTab from './MemoryBackgroundTab';

const hoisted = vi.hoisted(() => ({ list: vi.fn(), run: vi.fn() }));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryJobsList: (...a: unknown[]) => hoisted.list(...a),
  memoryJobsRun: (...a: unknown[]) => hoisted.run(...a),
}));

const JOBS: JobsList = {
  pending: [
    {
      id: 'j1',
      root: 'user:me',
      job: { job: 'build_beliefs' },
      queued_at: '2026-10-01T10:00:00Z',
      attempts: 2,
      last_error: 'engine timed out',
    },
    {
      id: 'j2',
      root: 'user:me',
      job: { job: 'ingest_brain', path: '/docs/a.pdf' },
      queued_at: '2026-10-01T10:01:00Z',
      attempts: 0,
    },
  ],
  history: [
    {
      id: 'r1',
      job: 'build_beliefs',
      root: 'user:me',
      ran_at: '2026-10-01T09:00:00Z',
      outcome: 'done',
      built: 3,
      stored: 3,
    },
    {
      id: 'r2',
      job: 'ingest_brain',
      root: 'user:me',
      ran_at: '2026-10-01T09:30:00Z',
      outcome: 'skipped',
      reason: 'already ingested',
      stored: 0,
    },
  ],
};

beforeEach(() => {
  hoisted.list.mockReset().mockResolvedValue(JOBS);
  hoisted.run.mockReset();
});

describe('MemoryBackgroundTab', () => {
  it('lists pending jobs and recent runs', async () => {
    renderWithProviders(<MemoryBackgroundTab />);
    const j1 = await screen.findByTestId('memory-job-j1');
    expect(j1).toHaveTextContent('Build beliefs');
    expect(j1).toHaveTextContent('2 attempts');
    expect(screen.getByTestId('memory-job-j1-error')).toHaveTextContent('engine timed out');
    expect(screen.getByTestId('memory-job-j2')).toHaveTextContent('Add to brain');
    expect(screen.getByTestId('memory-run-r1-outcome')).toHaveTextContent('Done');
    expect(screen.getByTestId('memory-run-r1')).toHaveTextContent('3 beliefs built');
    expect(screen.getByTestId('memory-run-r2-outcome')).toHaveTextContent('Skipped');
    expect(screen.getByTestId('memory-run-r2')).toHaveTextContent('already ingested');
  });

  it('runs every pending job and re-reads the queue', async () => {
    hoisted.run.mockResolvedValue({ runs: [JOBS.history[0], JOBS.history[1]] });
    renderWithProviders(<MemoryBackgroundTab />);
    fireEvent.click(await screen.findByTestId('memory-jobs-run-all'));
    await waitFor(() => expect(hoisted.run).toHaveBeenCalledWith(undefined));
    expect(await screen.findByTestId('memory-background-notice')).toHaveTextContent('Ran 2 jobs.');
    expect(hoisted.list).toHaveBeenCalledTimes(2);
  });

  it('runs one job', async () => {
    hoisted.run.mockResolvedValue({ runs: [JOBS.history[1]] });
    renderWithProviders(<MemoryBackgroundTab />);
    fireEvent.click(await screen.findByTestId('memory-job-j2-run'));
    await waitFor(() => expect(hoisted.run).toHaveBeenCalledWith('j2'));
  });

  it('refreshes on demand', async () => {
    renderWithProviders(<MemoryBackgroundTab />);
    fireEvent.click(await screen.findByTestId('memory-jobs-refresh'));
    await waitFor(() => expect(hoisted.list).toHaveBeenCalledTimes(2));
  });

  it('shows the empty queue and history, with Run all disabled', async () => {
    hoisted.list.mockResolvedValue({ pending: [], history: [] });
    renderWithProviders(<MemoryBackgroundTab />);
    expect(await screen.findByTestId('memory-jobs-empty')).toBeInTheDocument();
    expect(screen.getByTestId('memory-jobs-history-empty')).toBeInTheDocument();
    expect(screen.getByTestId('memory-jobs-run-all')).toBeDisabled();
  });

  it('shows a run failure', async () => {
    hoisted.run.mockRejectedValue(new Error('ENGINE: down'));
    renderWithProviders(<MemoryBackgroundTab />);
    fireEvent.click(await screen.findByTestId('memory-job-j1-run'));
    expect(await screen.findByTestId('memory-background-error')).toHaveTextContent('ENGINE: down');
  });

  it('shows a load error and a failed refresh', async () => {
    hoisted.list.mockRejectedValue(new Error('MEMORY_OFF'));
    renderWithProviders(<MemoryBackgroundTab />);
    expect(await screen.findByTestId('memory-background-error')).toHaveTextContent('MEMORY_OFF');
    fireEvent.click(screen.getByTestId('memory-jobs-refresh'));
    await waitFor(() => expect(hoisted.list).toHaveBeenCalledTimes(2));
    expect(screen.getByTestId('memory-background-error')).toBeInTheDocument();
  });
});
