import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../test/test-utils';
import MemoryMigrationBanner from './MemoryMigrationBanner';

const hoisted = vi.hoisted(() => ({
  scan: vi.fn(),
  start: vi.fn(),
  status: vi.fn(),
  retry: vi.fn(),
}));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryMigrationScan: (...a: unknown[]) => hoisted.scan(...a),
  memoryMigrationStart: (...a: unknown[]) => hoisted.start(...a),
  memoryMigrationStatus: (...a: unknown[]) => hoisted.status(...a),
  memoryMigrationRetry: (...a: unknown[]) => hoisted.retry(...a),
}));

const status = (phase: string, extra: object = {}, running = false) => ({
  state: { phase, copied: 0, ...extra },
  running,
  interrupted: false,
});

beforeEach(() => {
  hoisted.scan.mockReset().mockResolvedValue({ needed: true, shared: false });
  hoisted.status.mockReset().mockResolvedValue(status('idle'));
  hoisted.start.mockReset().mockResolvedValue(status('copying', { copied: 4 }, true));
  hoisted.retry.mockReset().mockResolvedValue({ phase: 'idle', copied: 0 });
});

describe('MemoryMigrationBanner', () => {
  it('renders nothing when there is nothing to move', async () => {
    hoisted.scan.mockResolvedValue({ needed: false, shared: false });
    renderWithProviders(<MemoryMigrationBanner />);
    await waitFor(() => expect(hoisted.scan).toHaveBeenCalled());
    expect(screen.queryByTestId('memory-migration-banner')).not.toBeInTheDocument();
  });

  it('migrates now without a dialog when the tree is the account’s own', async () => {
    renderWithProviders(<MemoryMigrationBanner />);
    fireEvent.click(await screen.findByTestId('memory-migration-start'));
    await waitFor(() => expect(hoisted.start).toHaveBeenCalledWith(false));
    expect(screen.queryByTestId('memory-migration-takeover')).not.toBeInTheDocument();
    expect(await screen.findByTestId('memory-migration-running')).toHaveTextContent('4');
  });

  it('takes a shared tree only after the takeover is confirmed', async () => {
    hoisted.scan.mockResolvedValue({ needed: true, shared: true });
    renderWithProviders(<MemoryMigrationBanner />);
    fireEvent.click(await screen.findByTestId('memory-migration-start'));
    expect(await screen.findByTestId('memory-migration-takeover')).toBeInTheDocument();
    expect(hoisted.start).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('memory-migration-takeover-cancel'));
    expect(hoisted.start).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId('memory-migration-start'));
    fireEvent.click(await screen.findByTestId('memory-migration-takeover-confirm'));
    await waitFor(() => expect(hoisted.start).toHaveBeenCalledWith(true));
  });

  it('offers to try again what could not be moved', async () => {
    hoisted.scan.mockResolvedValue({ needed: false, shared: false });
    hoisted.status.mockResolvedValue(
      status('cleaned', { failures: [{ id: 'a', reason: 'too_large' }], incomplete: ['b'] })
    );
    renderWithProviders(<MemoryMigrationBanner />);
    expect(await screen.findByTestId('memory-migration-left')).toHaveTextContent('2');
    fireEvent.click(screen.getByTestId('memory-migration-retry'));
    await waitFor(() => expect(hoisted.retry).toHaveBeenCalled());
    await waitFor(() => expect(hoisted.start).toHaveBeenCalledWith(false));
  });
});
