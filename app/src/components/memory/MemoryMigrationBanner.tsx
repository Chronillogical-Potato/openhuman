/**
 * "Move memory into your account": when `memory_migration_scan` finds memory
 * from before the per-user layout, say so. The move runs on its own in the
 * background while it is free; "Migrate now" starts it at once. A legacy tree
 * other accounts on this machine may share (self-hosted CortexDB) is only
 * taken after the takeover dialog's confirmation, the only caller of
 * `memory_migration_start({takeover: true})`. Items that could not be moved
 * stay where they were, and the banner offers to try them again.
 *
 * debug logging: DEBUG=openhuman:memory:migration
 */
import debug from 'debug';
import { useCallback, useEffect, useRef, useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import {
  memoryErrorMessage,
  memoryMigrationRetry,
  memoryMigrationScan,
  memoryMigrationStart,
  memoryMigrationStatus,
  type MigrationScan,
  type MigrationStatus,
} from '../../services/api/memoryApi';
import { Alert, AlertDescription, AlertTitle, Button, ConfirmDialog } from '../ui';
import MemoryErrorAlert from './MemoryErrorAlert';
import { fill } from './memoryFormat';

const log = debug('openhuman:memory:migration');

/** How often a running move is polled. */
export const MIGRATION_POLL_MS = 2_000;
/** How often an offered move is polled, to notice the background job start it. */
export const MIGRATION_IDLE_POLL_MS = 15_000;

export default function MemoryMigrationBanner() {
  const { t } = useT();
  const [scan, setScan] = useState<MigrationScan | null>(null);
  const [status, setStatus] = useState<MigrationStatus | null>(null);
  const [takeoverOpen, setTakeoverOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([memoryMigrationScan(), memoryMigrationStatus().catch(() => null)])
      .then(([found, current]) => {
        if (cancelled) return;
        log('scan: needed=%s shared=%s', found?.needed, found?.shared);
        setScan(found ?? null);
        setStatus(current ?? null);
      })
      .catch(err => {
        // A failed scan only hides the banner; it is not worth an error.
        log('scan failed: %o', err);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // Whether the last status seen had a run going.
  const wasRunning = useRef(false);
  const poll = useCallback(async () => {
    try {
      const next = await memoryMigrationStatus();
      setStatus(next);
      // A run just ended: whether anything is still left to move changed.
      // (The scan reads the legacy tree, so it is not repeated otherwise.)
      if (wasRunning.current && !next.running) setScan(await memoryMigrationScan());
      wasRunning.current = next.running;
    } catch (err) {
      log('status failed: %o', err);
      setError(memoryErrorMessage(err, t));
    }
  }, [t]);

  const running = status?.running ?? false;
  const offered = scan?.needed ?? false;
  useEffect(() => {
    wasRunning.current = running;
  }, [running]);
  useEffect(() => {
    // While a run goes, and while the move is offered: the background job
    // can start it at any time.
    if (!running && !offered) return;
    const timer = setInterval(
      () => void poll(),
      running ? MIGRATION_POLL_MS : MIGRATION_IDLE_POLL_MS
    );
    return () => clearInterval(timer);
  }, [running, offered, poll]);

  const start = async (takeover: boolean) => {
    setBusy(true);
    setError(null);
    try {
      const next = await memoryMigrationStart(takeover);
      log('start: takeover=%s phase=%s running=%s', takeover, next.state.phase, next.running);
      setStatus(next);
    } catch (err) {
      log('start failed: %o', err);
      setError(memoryErrorMessage(err, t));
    } finally {
      setBusy(false);
      setTakeoverOpen(false);
    }
  };

  const retry = async () => {
    setBusy(true);
    setError(null);
    try {
      await memoryMigrationRetry();
      setStatus(await memoryMigrationStart(false));
    } catch (err) {
      log('retry failed: %o', err);
      setError(memoryErrorMessage(err, t));
    } finally {
      setBusy(false);
    }
  };

  const state = status?.state;
  const left = (state?.failures?.length ?? 0) + (state?.incomplete?.length ?? 0);
  const cleaned = state?.phase === 'cleaned';
  // A retry of what was left runs while the scan says nothing is needed.
  if (!running && !offered && !(cleaned && left > 0)) return null;

  const migrateNow = () => (scan?.shared ? setTakeoverOpen(true) : void start(false));
  const paused = !running && (state?.phase === 'paused' || status?.interrupted);

  return (
    <div data-testid="memory-migration-banner">
      {running ? (
        <Alert variant="info" data-testid="memory-migration-running">
          <div className="w-full space-y-1">
            <AlertTitle>{t('memoryPage.migrate.running')}</AlertTitle>
            <AlertDescription>
              {fill(
                t(
                  state?.copied === 1
                    ? 'memoryPage.migrate.progressOne'
                    : 'memoryPage.migrate.progress'
                ),
                { copied: state?.copied ?? 0 }
              )}
            </AlertDescription>
          </div>
        </Alert>
      ) : cleaned ? (
        <Alert variant="warning" data-testid="memory-migration-left">
          <div className="flex w-full flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <AlertTitle>
                {fill(
                  t(
                    left === 1 ? 'memoryPage.migrate.leftTitleOne' : 'memoryPage.migrate.leftTitle'
                  ),
                  { count: left }
                )}
              </AlertTitle>
              <AlertDescription>{t('memoryPage.migrate.leftBody')}</AlertDescription>
            </div>
            <Button
              type="button"
              size="sm"
              variant="primary"
              disabled={busy}
              data-testid="memory-migration-retry"
              onClick={() => void retry()}>
              {t('memoryPage.migrate.retry')}
            </Button>
          </div>
        </Alert>
      ) : (
        <Alert
          variant="info"
          data-testid={paused ? 'memory-migration-paused' : 'memory-migration-offer'}>
          <div className="flex w-full flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <AlertTitle>
                {paused ? t('memoryPage.migrate.paused') : t('memoryPage.migrate.title')}
              </AlertTitle>
              <AlertDescription>
                {paused && state?.error ? state.error : t('memoryPage.migrate.body')}
              </AlertDescription>
            </div>
            <Button
              type="button"
              size="sm"
              variant="primary"
              disabled={busy}
              data-testid="memory-migration-start"
              onClick={migrateNow}>
              {paused ? t('memoryPage.migrate.resume') : t('memoryPage.migrate.action')}
            </Button>
          </div>
        </Alert>
      )}

      {error !== null && (
        <MemoryErrorAlert message={error} className="mt-3" data-testid="memory-migration-error" />
      )}

      {takeoverOpen && (
        <ConfirmDialog
          title={t('memoryPage.migrate.takeoverTitle')}
          testId="memory-migration-takeover"
          confirmTestId="memory-migration-takeover-confirm"
          cancelTestId="memory-migration-takeover-cancel"
          busy={busy}
          confirmLabel={t('memoryPage.migrate.takeoverConfirm')}
          body={
            <p className="text-sm text-content-secondary">{t('memoryPage.migrate.takeoverBody')}</p>
          }
          onConfirm={() => void start(true)}
          onCancel={() => setTakeoverOpen(false)}
        />
      )}
    </div>
  );
}
