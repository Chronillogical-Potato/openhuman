/**
 * A one-time banner on Memory → Provider announcing the move from TinyCortex
 * to CortexDB. Dismissing it is remembered per user (`userScopedStorage`), so
 * it never comes back for that account.
 *
 * debug logging: DEBUG=openhuman:memory:announcement
 */
import debug from 'debug';
import { X } from 'lucide-react';
import { useEffect, useState } from 'react';

import cortexdbLogo from '../../assets/provider-icons/cortexdb.png';
import { useT } from '../../lib/i18n/I18nContext';
import { userScopedStorage } from '../../store/userScopedStorage';
import { Button } from '../ui';

const log = debug('openhuman:memory:announcement');

/** Storage key for the dismissal; bump the suffix to show a new announcement. */
export const CORTEX_ANNOUNCEMENT_KEY = 'memory.cortexdbAnnouncement.dismissed.v1';

export default function MemoryCortexAnnouncement() {
  const { t } = useT();
  // Unknown until storage answers, so a dismissed banner never flashes.
  const [dismissed, setDismissed] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    userScopedStorage
      .getItem(CORTEX_ANNOUNCEMENT_KEY)
      .then(value => {
        if (!cancelled) setDismissed(value === 'true');
      })
      .catch(err => {
        log('read failed: %o', err);
        if (!cancelled) setDismissed(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  if (dismissed !== false) return null;

  const dismiss = () => {
    setDismissed(true);
    log('dismissed');
    void userScopedStorage.setItem(CORTEX_ANNOUNCEMENT_KEY, 'true');
  };

  return (
    <section
      role="status"
      aria-labelledby="memory-cortex-announcement-title"
      data-testid="memory-cortex-announcement"
      className="relative overflow-hidden rounded-xl border border-primary-500/30 bg-gradient-to-br from-primary-500/10 via-primary-500/5 to-transparent p-4 pr-11">
      <div className="flex items-start gap-3">
        <span className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-white ring-1 ring-line">
          <img src={cortexdbLogo} alt="" aria-hidden className="h-6 w-6 object-contain" />
        </span>
        <div className="min-w-0 flex-1 space-y-1.5">
          <h3 id="memory-cortex-announcement-title" className="text-sm font-semibold text-content">
            {t('memoryPage.announcement.title')}
          </h3>
          <p className="text-xs leading-relaxed text-content-secondary">
            {t('memoryPage.announcement.retired')}
          </p>
          <p className="text-xs leading-relaxed text-content-secondary">
            {t('memoryPage.announcement.subscribers')}
          </p>
        </div>
      </div>
      <Button
        size="xs"
        variant="tertiary"
        iconOnly
        analyticsId="memory-cortex-announcement-dismiss"
        data-testid="memory-cortex-announcement-dismiss"
        aria-label={t('memoryPage.announcement.dismiss')}
        className="absolute top-3 right-3 text-content-muted hover:text-content"
        onClick={dismiss}>
        <X className="h-3.5 w-3.5" aria-hidden />
      </Button>
    </section>
  );
}
