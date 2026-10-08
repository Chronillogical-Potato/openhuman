/**
 * Memory → Migration: everything about bringing memory forward from earlier
 * versions, in one place instead of a banner over every chip. Holds the
 * CortexDB announcement and the import-then-organize flow
 * (`MemoryImportBanner`); with nothing left to import or move it says so.
 */
import { LuDatabaseBackup } from 'react-icons/lu';

import { useT } from '../../lib/i18n/I18nContext';
import EmptyStateCard from '../EmptyStateCard';
import MemoryCortexAnnouncement from './MemoryCortexAnnouncement';
import MemoryImportBanner from './MemoryImportBanner';

interface MemoryMigrationTabProps {
  /** Label of the engine imported memory is uploaded to. */
  engineLabel: string;
}

export default function MemoryMigrationTab({ engineLabel }: MemoryMigrationTabProps) {
  const { t } = useT();
  return (
    <div className="w-full space-y-5 animate-fade-up" data-testid="memory-migration-tab">
      <MemoryCortexAnnouncement />
      <MemoryImportBanner
        engineLabel={engineLabel}
        emptyState={
          <div data-testid="memory-migration-empty">
            <EmptyStateCard
              icon={<LuDatabaseBackup className="h-6 w-6 text-primary-500" aria-hidden />}
              title={t('memoryPage.migrate.nothingTitle')}
              description={t('memoryPage.migrate.nothingBody')}
            />
          </div>
        }
      />
    </div>
  );
}
