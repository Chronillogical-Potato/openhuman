import { screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../test/test-utils';
import MemoryMigrationTab from './MemoryMigrationTab';

vi.mock('./MemoryCortexAnnouncement', () => ({
  default: () => <div data-testid="stub-announcement" />,
}));
vi.mock('./MemoryImportBanner', () => ({
  default: ({ engineLabel, emptyState }: { engineLabel: string; emptyState: React.ReactNode }) => (
    <div data-testid="stub-import">
      {engineLabel}
      {emptyState}
    </div>
  ),
}));

describe('MemoryMigrationTab', () => {
  it('shows the announcement and the import flow with its empty state', () => {
    renderWithProviders(<MemoryMigrationTab engineLabel="TinyHumans" />);
    expect(screen.getByTestId('stub-announcement')).toBeInTheDocument();
    expect(screen.getByTestId('stub-import')).toHaveTextContent('TinyHumans');
    expect(screen.getByTestId('memory-migration-empty')).toHaveTextContent('Nothing to migrate');
  });

  it('shows the off state in place of the import flow', () => {
    renderWithProviders(
      <MemoryMigrationTab engineLabel="TinyHumans" offState={<div data-testid="off" />} />
    );
    expect(screen.getByTestId('stub-announcement')).toBeInTheDocument();
    expect(screen.getByTestId('off')).toBeInTheDocument();
    expect(screen.queryByTestId('stub-import')).not.toBeInTheDocument();
  });
});
