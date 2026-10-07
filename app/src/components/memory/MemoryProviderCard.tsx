import { Gift, Settings } from 'lucide-react';
import type { ReactNode } from 'react';

import { cn } from '../../lib/cn';
import { useT } from '../../lib/i18n/I18nContext';
import { IncludedTag } from '../settings/panels/LiveVoiceVendorCard';
import { Badge, type BadgeVariant, Button } from '../ui';
import MemoryProviderLogo, { type MemoryProviderOption } from './MemoryProviderLogo';

export interface MemoryProviderCardProps {
  option: MemoryProviderOption;
  title: string;
  description: string;
  /** True when this is the configured provider (even if it is off or down). */
  active: boolean;
  status: { variant: BadgeVariant; label: string };
  /** The card's one call to action; omitted when there is nothing to do. */
  action?: { label: string; onClick: () => void; disabled?: boolean };
  /** Small note under the tags (the free-ingestion line on TinyHumans). */
  note?: ReactNode;
  onOpenSettings: () => void;
}

/** One way to store memory on Memory → Provider: logo, what it is, status and actions. */
const MemoryProviderCard = ({
  option,
  title,
  description,
  active,
  status,
  action,
  note,
  onOpenSettings,
}: MemoryProviderCardProps) => {
  const { t } = useT();
  return (
    <div
      data-testid={`memory-engine-${option}`}
      data-active={active || undefined}
      className={cn(
        'flex h-full flex-col gap-3 rounded-xl border p-4 transition-colors',
        active
          ? 'border-primary-500 bg-primary-50 ring-1 ring-primary-500 dark:bg-primary-500/10'
          : 'border-line bg-surface'
      )}>
      <div className="flex items-start gap-3">
        <span
          className={cn(
            'flex h-11 w-11 shrink-0 items-center justify-center rounded-xl',
            option === 'builtin'
              ? active
                ? 'bg-primary-500 text-content-inverted'
                : 'bg-surface-strong text-content'
              : 'bg-white ring-1 ring-line'
          )}>
          <MemoryProviderLogo option={option} className="h-6 w-6" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center justify-between gap-x-2 gap-y-1">
            <h4 className="text-sm font-semibold text-content">{title}</h4>
            <Badge variant={status.variant} data-testid={`memory-engine-${option}-status`}>
              {status.label}
            </Badge>
          </div>
          <p className="mt-0.5 text-xs leading-relaxed text-content-muted">{description}</p>
        </div>
      </div>

      {note && (
        <p
          className="flex items-center gap-1.5 text-xs text-content-secondary"
          data-testid={`memory-engine-${option}-note`}>
          <Gift className="h-3.5 w-3.5 shrink-0 text-primary-500" aria-hidden />
          {note}
        </p>
      )}

      <div className="mt-auto flex flex-wrap items-center justify-between gap-2">
        {option === 'builtin' ? (
          <IncludedTag label={t('memoryPage.engine.includedTag')} />
        ) : (
          <Badge>
            {option === 'apikey'
              ? t('memoryPage.engine.tagOwnKey')
              : t('memoryPage.engine.tagThisComputer')}
          </Badge>
        )}
        <div className="ml-auto flex items-center gap-1.5">
          {action && (
            <Button
              size="sm"
              analyticsId={`memory-engine-${option}-action`}
              data-testid={`memory-engine-${option}-action`}
              disabled={action.disabled}
              onClick={action.onClick}>
              {action.label}
            </Button>
          )}
          <Button
            size="sm"
            variant="secondary"
            iconOnly
            analyticsId="memory-engine-open-settings"
            data-testid={`memory-engine-${option}-settings`}
            aria-label={t('memoryPage.engine.settingsAria').replace('{name}', title)}
            title={t('memoryPage.engine.settings')}
            onClick={onOpenSettings}>
            <Settings className="h-4 w-4" aria-hidden />
          </Button>
        </div>
      </div>
    </div>
  );
};

export default MemoryProviderCard;
