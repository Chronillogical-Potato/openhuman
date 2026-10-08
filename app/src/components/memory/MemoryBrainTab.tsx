/**
 * Memory → Files: everything that feeds memory from files on this device.
 *
 * - The CortexDB announcement and the import-then-organize flow for memory
 *   from earlier versions ({@link MemoryImportBanner}), on top.
 * - A manual "Add document" upload from pasted text or a file path
 *   (`memory_brain_ingest`).
 * - The synced folder / file sources that keep feeding memory
 *   ({@link MemorySyncedSources}).
 *
 * While memory is off the import flow and the sources are replaced by
 * `offState`, which points at the Provider tab.
 *
 * debug logging: DEBUG=openhuman:memory:brain
 */
import debug from 'debug';
import { type ReactNode, useState } from 'react';
import { LuPlus } from 'react-icons/lu';

import { useT } from '../../lib/i18n/I18nContext';
import {
  type BrainIngestRequest,
  memoryBrainIngest,
  memoryErrorMessage,
} from '../../services/api/memoryApi';
import { Alert, AlertDescription, Button, Card } from '../ui';
import MemoryBrainIngestDialog from './MemoryBrainIngestDialog';
import MemoryCortexAnnouncement from './MemoryCortexAnnouncement';
import MemoryErrorAlert from './MemoryErrorAlert';
import { fill } from './memoryFormat';
import MemoryImportBanner from './MemoryImportBanner';
import { brainSourceLabel } from './memoryLifecycleLabels';
import MemorySyncedSources from './MemorySyncedSources';

const log = debug('openhuman:memory:brain');

interface MemoryBrainTabProps {
  /** Label of the engine imported memory is uploaded to. */
  engineLabel: string;
  /** Shown in place of the import flow, upload and sources while memory is off. */
  offState?: ReactNode;
}

export default function MemoryBrainTab({ engineLabel, offState }: MemoryBrainTabProps) {
  const { t } = useT();
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const ingest = async (req: BrainIngestRequest): Promise<boolean> => {
    setSaving(true);
    setAddError(null);
    setNotice(null);
    setError(null);
    try {
      const res = await memoryBrainIngest(req);
      log('ingested id=%s source=%s replayed=%s', res.id, res.source, res.replayed);
      setNotice(
        res.replayed
          ? t('memoryPage.brain.ingestReplayed')
          : fill(t('memoryPage.brain.ingestDone'), { source: brainSourceLabel(res.source, t) })
      );
      return true;
    } catch (err) {
      log('brain_ingest failed: %o', err);
      setAddError(memoryErrorMessage(err, t));
      return false;
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-4 animate-fade-up" data-testid="memory-brain-tab">
      <MemoryCortexAnnouncement />
      {offState ?? (
        <>
          <MemoryImportBanner engineLabel={engineLabel} />

          {error !== null && <MemoryErrorAlert message={error} data-testid="memory-brain-error" />}
          {notice !== null && (
            <Alert variant="success" data-testid="memory-brain-notice">
              <AlertDescription>{notice}</AlertDescription>
            </Alert>
          )}

          <Card
            title={t('memoryPage.brain.ingestTitle')}
            description={t('memoryPage.brain.ingestSubtitle')}
            headerRight={
              <Button
                type="button"
                variant="primary"
                size="sm"
                analyticsId="memory-brain-add-document"
                data-testid="memory-brain-add"
                onClick={() => {
                  setAddError(null);
                  setAdding(true);
                }}>
                <LuPlus className="h-3.5 w-3.5" aria-hidden />
                {t('memoryPage.brain.addDocument')}
              </Button>
            }
            data-testid="memory-brain-upload">
            {null}
          </Card>

          <MemorySyncedSources />

          {adding && (
            <MemoryBrainIngestDialog
              saving={saving}
              error={addError}
              onSubmit={ingest}
              onClose={() => setAdding(false)}
            />
          )}
        </>
      )}
    </div>
  );
}
