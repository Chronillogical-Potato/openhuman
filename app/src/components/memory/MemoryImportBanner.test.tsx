import { act, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../test/test-utils';
import MemoryImportBanner, { IMPORT_POLL_MS } from './MemoryImportBanner';

const hoisted = vi.hoisted(() => ({
  scan: vi.fn(),
  start: vi.fn(),
  status: vi.fn(),
  retry: vi.fn(),
  mScan: vi.fn(),
  mStart: vi.fn(),
  mStatus: vi.fn(),
  mRetry: vi.fn(),
}));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryImportScan: (...a: unknown[]) => hoisted.scan(...a),
  memoryImportStart: (...a: unknown[]) => hoisted.start(...a),
  memoryImportStatus: (...a: unknown[]) => hoisted.status(...a),
  memoryImportRetryFailed: (...a: unknown[]) => hoisted.retry(...a),
  memoryMigrationScan: (...a: unknown[]) => hoisted.mScan(...a),
  memoryMigrationStart: (...a: unknown[]) => hoisted.mStart(...a),
  memoryMigrationStatus: (...a: unknown[]) => hoisted.mStatus(...a),
  memoryMigrationRetry: (...a: unknown[]) => hoisted.mRetry(...a),
}));

const moving = (copied: number) => ({
  state: { phase: 'copying', copied },
  running: true,
  interrupted: false,
});
const MOVE_IDLE = { state: { phase: 'idle', copied: 0 }, running: false, interrupted: false };

const IDLE = { state: { phase: 'idle', imported: 0, total: 0 } };
const FOUND = { found: true, counts: { documents: 3, conversations: 5, learnings: 2 } };

beforeEach(() => {
  hoisted.scan.mockReset().mockResolvedValue(FOUND);
  hoisted.status.mockReset().mockResolvedValue(IDLE);
  hoisted.start.mockReset();
  hoisted.retry.mockReset();
  hoisted.mScan.mockReset().mockResolvedValue({ needed: false, shared: false });
  hoisted.mStatus.mockReset().mockResolvedValue(MOVE_IDLE);
  hoisted.mStart.mockReset().mockResolvedValue(moving(4));
  hoisted.mRetry.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('MemoryImportBanner', () => {
  it('renders nothing when there is nothing to import', async () => {
    hoisted.scan.mockResolvedValue({ found: false });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    await waitFor(() => expect(hoisted.scan).toHaveBeenCalled());
    expect(screen.queryByTestId('memory-import-banner')).not.toBeInTheDocument();
  });

  it('offers the import with the counts it found', async () => {
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-counts')).toHaveTextContent(
      '3 documents, 5 conversations and 2 learnings'
    );
  });

  it('asks for consent naming the engine, and uploads nothing on cancel', async () => {
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    fireEvent.click(await screen.findByTestId('memory-import-open'));
    expect(screen.getByTestId('memory-import-consent')).toHaveTextContent(
      'uploads it to TinyHumans'
    );
    fireEvent.click(screen.getByTestId('memory-import-cancel'));
    expect(screen.queryByTestId('memory-import-consent')).not.toBeInTheDocument();
    expect(hoisted.start).not.toHaveBeenCalled();
  });

  it('starts the import on consent and polls progress until done', async () => {
    hoisted.start.mockResolvedValue({ state: { phase: 'running', imported: 0, total: 10 } });
    renderWithProviders(<MemoryImportBanner engineLabel="CortexDB" />);
    fireEvent.click(await screen.findByTestId('memory-import-open'));

    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.click(screen.getByTestId('memory-import-confirm'));
    expect(await screen.findByTestId('memory-import-running')).toHaveTextContent(
      '0 of 10 items imported'
    );
    expect(hoisted.start).toHaveBeenCalledTimes(1);

    hoisted.status.mockResolvedValue({ state: { phase: 'done', imported: 10, total: 10 } });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(IMPORT_POLL_MS + 10);
    });
    expect(await screen.findByTestId('memory-import-done')).toHaveTextContent(
      '10 of 10 items imported'
    );
    expect(screen.queryByTestId('memory-import-open')).not.toBeInTheDocument();
  });

  it('offers to retry the items a finished import could not store', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'done', imported: 8, total: 9, failed: 1 },
    });
    hoisted.retry.mockResolvedValue({
      state: { phase: 'running', imported: 8, total: 9, failed: 1 },
    });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);

    expect(await screen.findByTestId('memory-import-failed-items')).toHaveTextContent(
      'Items that could not be imported: 1.'
    );
    fireEvent.click(screen.getByTestId('memory-import-retry-failed'));
    expect(await screen.findByTestId('memory-import-running')).toBeInTheDocument();
    expect(hoisted.retry).toHaveBeenCalledTimes(1);
    expect(hoisted.start).not.toHaveBeenCalled();
  });

  it('says why a retry stopped and keeps offering it', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'done', imported: 8, total: 9, failed: 1, error: 'sign in to continue' },
    });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-failed-items')).toHaveTextContent(
      'sign in to continue'
    );
    expect(screen.getByTestId('memory-import-retry-failed')).toBeEnabled();
  });

  it('offers no retry when nothing failed', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'done', imported: 9, total: 9, failed: 0 },
    });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-done')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-import-retry-failed')).not.toBeInTheDocument();
  });

  it('shows a retry that could not start', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'done', imported: 8, total: 9, failed: 1 },
    });
    hoisted.retry.mockRejectedValue(new Error('no failed items to retry'));
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    fireEvent.click(await screen.findByTestId('memory-import-retry-failed'));
    expect(await screen.findByTestId('memory-import-error')).toBeInTheDocument();
  });

  it('shows a failed import', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'error', imported: 2, total: 9, error: 'engine rejected batch' },
    });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-error')).toHaveTextContent(
      'engine rejected batch'
    );
  });

  it('resumes a failed import through the consent dialog', async () => {
    hoisted.status.mockResolvedValue({
      state: { phase: 'error', imported: 2, total: 9, error: 'not enough credits to import' },
    });
    hoisted.start.mockResolvedValue({ state: { phase: 'running', imported: 2, total: 9 } });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);

    fireEvent.click(await screen.findByTestId('memory-import-resume'));
    expect(screen.getByTestId('memory-import-consent')).toBeInTheDocument();
    expect(hoisted.start).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('memory-import-confirm'));
    expect(await screen.findByTestId('memory-import-running')).toHaveTextContent(
      '2 of 9 items imported'
    );
    expect(hoisted.start).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('memory-import-resume')).not.toBeInTheDocument();
  });

  it('offers no resume while an import is running', async () => {
    hoisted.status.mockResolvedValue({ state: { phase: 'running', imported: 1, total: 9 } });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-running')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-import-resume')).not.toBeInTheDocument();
  });

  it('shows a start failure', async () => {
    hoisted.start.mockRejectedValue(new Error('UNAUTHORIZED: sign in again'));
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    fireEvent.click(await screen.findByTestId('memory-import-open'));
    fireEvent.click(screen.getByTestId('memory-import-confirm'));
    expect(await screen.findByTestId('memory-import-error')).toHaveTextContent('sign in again');
  });

  it('does not organize while the import is still running', async () => {
    hoisted.status.mockResolvedValue({ state: { phase: 'running', imported: 1, total: 9 } });
    hoisted.mScan.mockResolvedValue({ needed: true, shared: false });
    renderWithProviders(<MemoryImportBanner engineLabel="TinyHumans" />);
    expect(await screen.findByTestId('memory-import-running')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-migration-banner')).not.toBeInTheDocument();
    expect(hoisted.mStart).not.toHaveBeenCalled();
  });

  it('takes a shared tree only after the takeover is confirmed', async () => {
    hoisted.scan.mockResolvedValue({ found: false });
    hoisted.mScan.mockResolvedValue({ needed: true, shared: true });
    renderWithProviders(<MemoryImportBanner engineLabel="CortexDB" />);
    fireEvent.click(await screen.findByTestId('memory-migration-start'));
    expect(await screen.findByTestId('memory-migration-takeover')).toBeInTheDocument();
    expect(hoisted.mStart).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId('memory-migration-takeover-confirm'));
    await waitFor(() => expect(hoisted.mStart).toHaveBeenCalledWith(true));
  });
});
