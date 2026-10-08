import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../test/test-utils';
import MemoryBrainTab from './MemoryBrainTab';

const hoisted = vi.hoisted(() => ({ ingest: vi.fn() }));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryBrainIngest: (...a: unknown[]) => hoisted.ingest(...a),
}));

// The synced-sources registry, the import flow and the announcement have their own suites.
vi.mock('./MemorySyncedSources', () => ({
  default: () => <div data-testid="stub-synced-sources" />,
}));
vi.mock('./MemoryImportBanner', () => ({
  default: ({ engineLabel }: { engineLabel: string }) => (
    <div data-testid="stub-import">{engineLabel}</div>
  ),
}));
vi.mock('./MemoryCortexAnnouncement', () => ({
  default: () => <div data-testid="stub-announcement" />,
}));

beforeEach(() => {
  hoisted.ingest.mockReset();
});

describe('MemoryBrainTab (Files)', () => {
  it('shows the announcement and import banner on top, then upload and synced sources', () => {
    renderWithProviders(<MemoryBrainTab engineLabel="TinyHumans" />);
    expect(screen.getByTestId('stub-announcement')).toBeInTheDocument();
    expect(screen.getByTestId('stub-import')).toHaveTextContent('TinyHumans');
    expect(screen.getByTestId('memory-brain-add')).toBeInTheDocument();
    expect(screen.getByTestId('stub-synced-sources')).toBeInTheDocument();
    // The per-source counts, search and forget-source block are gone.
    expect(screen.queryByTestId('memory-brain-sources')).not.toBeInTheDocument();
    expect(screen.queryByTestId('memory-brain-search-input')).not.toBeInTheDocument();
  });

  it('replaces the import flow, upload and sources with the off state while memory is off', () => {
    renderWithProviders(
      <MemoryBrainTab engineLabel="TinyHumans" offState={<div data-testid="off" />} />
    );
    expect(screen.getByTestId('stub-announcement')).toBeInTheDocument();
    expect(screen.getByTestId('off')).toBeInTheDocument();
    expect(screen.queryByTestId('stub-import')).not.toBeInTheDocument();
    expect(screen.queryByTestId('memory-brain-add')).not.toBeInTheDocument();
    expect(screen.queryByTestId('stub-synced-sources')).not.toBeInTheDocument();
  });

  it('adds pasted text with a title and source', async () => {
    hoisted.ingest.mockResolvedValue({ id: 'doc-1', source: 'markdown', replayed: false });
    renderWithProviders(<MemoryBrainTab engineLabel="TinyHumans" />);
    fireEvent.click(screen.getByTestId('memory-brain-add'));
    const submit = screen.getByTestId('memory-brain-ingest-submit');
    expect(submit).toBeDisabled();

    fireEvent.change(screen.getByTestId('memory-brain-ingest-text'), {
      target: { value: '  # Notes  ' },
    });
    fireEvent.change(screen.getByTestId('memory-brain-ingest-title'), {
      target: { value: 'Notes' },
    });
    fireEvent.change(screen.getByTestId('memory-brain-ingest-source'), {
      target: { value: 'markdown' },
    });
    fireEvent.click(submit);

    await waitFor(() =>
      expect(hoisted.ingest).toHaveBeenCalledWith({
        text: '# Notes',
        title: 'Notes',
        source: 'markdown',
      })
    );
    expect(await screen.findByTestId('memory-brain-notice')).toHaveTextContent(
      'Added to the brain under Markdown.'
    );
    expect(screen.queryByTestId('memory-brain-ingest')).not.toBeInTheDocument();
  });

  it('adds a file by path and reports a replayed document', async () => {
    hoisted.ingest.mockResolvedValue({ id: 'doc-1', source: 'pdf', replayed: true });
    renderWithProviders(<MemoryBrainTab engineLabel="TinyHumans" />);
    fireEvent.click(screen.getByTestId('memory-brain-add'));
    fireEvent.click(screen.getByTestId('memory-brain-ingest-from-path'));
    fireEvent.change(screen.getByTestId('memory-brain-ingest-path'), {
      target: { value: '/docs/plan.pdf' },
    });
    fireEvent.click(screen.getByTestId('memory-brain-ingest-submit'));
    await waitFor(() => expect(hoisted.ingest).toHaveBeenCalledWith({ path: '/docs/plan.pdf' }));
    expect(await screen.findByTestId('memory-brain-notice')).toHaveTextContent(
      'That document is already in the brain.'
    );
  });

  it('keeps the dialog open with the error when ingest fails', async () => {
    hoisted.ingest.mockRejectedValue(new Error('INVALID_REQUEST: unreadable'));
    renderWithProviders(<MemoryBrainTab engineLabel="TinyHumans" />);
    fireEvent.click(screen.getByTestId('memory-brain-add'));
    fireEvent.change(screen.getByTestId('memory-brain-ingest-text'), { target: { value: 'x' } });
    fireEvent.click(screen.getByTestId('memory-brain-ingest-submit'));
    expect(await screen.findByTestId('memory-brain-ingest-error')).toHaveTextContent('unreadable');
    expect(screen.getByTestId('memory-brain-ingest')).toBeInTheDocument();
  });

  it('prompts a top-up in the ingest dialog when the account is out of credits', async () => {
    hoisted.ingest.mockRejectedValue(new Error('INSUFFICIENT_CREDITS: HTTP 402'));
    renderWithProviders(<MemoryBrainTab engineLabel="TinyHumans" />);
    fireEvent.click(screen.getByTestId('memory-brain-add'));
    fireEvent.change(screen.getByTestId('memory-brain-ingest-text'), { target: { value: 'x' } });
    fireEvent.click(screen.getByTestId('memory-brain-ingest-submit'));
    const prompt = await screen.findByTestId('memory-brain-ingest-error');
    expect(prompt).toHaveAttribute('data-kind', 'out-of-credits');
    expect(screen.getByTestId('memory-top-up')).toBeInTheDocument();
  });
});
