import { Settings2, Sparkles } from 'lucide-react';

import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import Badge from '../../ui/Badge';
import Button from '../../ui/Button';
import { type LiveVoiceVendor, vendorDescKey, vendorIcon } from './liveVoiceVendors';

/** The highlighted "Included with TinyHumans" tag on managed voice agents. */
export const IncludedTag = ({ className }: { className?: string }) => {
  const { t } = useT();
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full bg-primary-500/10 px-2 py-0.5 text-[11px] font-medium text-primary-600 ring-1 ring-primary-500/20 ring-inset dark:text-primary-300',
        className
      )}>
      <Sparkles className="h-3 w-3" aria-hidden />
      {t('connections.voiceAgents.includedTag')}
    </span>
  );
};

export interface LiveVoiceVendorCardProps {
  vendor: LiveVoiceVendor;
  /** The provider id the live voice agent currently talks through. */
  defaultProvider: string;
  saving: boolean;
  /** Make this vendor's preferred ready provider the one in use. */
  onUse: (providerId: string) => void;
  onOpenSettings: () => void;
}

/**
 * One voice service on Connections → Voice agents: what it is, whether it
 * comes with TinyHumans, whether it is the one in use, and the way into its
 * settings.
 */
const LiveVoiceVendorCard = ({
  vendor,
  defaultProvider,
  saving,
  onUse,
  onOpenSettings,
}: LiveVoiceVendorCardProps) => {
  const { t } = useT();
  const Icon = vendorIcon(vendor.id);
  const descKey = vendorDescKey(vendor.id);
  const inUse = vendor.providers.some(p => p.id === defaultProvider);
  const hasHosted = vendor.providers.some(p => p.kind === 'hosted');
  const ready = vendor.providers.find(p => p.configured);

  return (
    <div
      data-testid={`live-voice-vendor-${vendor.id}`}
      data-in-use={inUse || undefined}
      className={cn(
        'flex h-full flex-col rounded-xl border p-4 transition-colors',
        inUse
          ? 'border-primary-500 bg-primary-50 ring-1 ring-primary-500 dark:bg-primary-500/10'
          : 'border-line bg-surface'
      )}>
      <div className="flex items-start justify-between gap-2">
        <span
          aria-hidden
          className={cn(
            'flex h-10 w-10 shrink-0 items-center justify-center rounded-xl [&_svg]:h-5 [&_svg]:w-5',
            inUse ? 'bg-primary-500 text-content-inverted' : 'bg-surface-strong text-content'
          )}>
          <Icon />
        </span>
        {inUse ? (
          <Badge variant="primary">{t('connections.voiceAgents.badgeInUse')}</Badge>
        ) : ready ? (
          <Badge variant="success">{t('connections.voiceAgents.badgeReady')}</Badge>
        ) : (
          <Badge variant="warning">{t('connections.voiceAgents.badgeNeedsKey')}</Badge>
        )}
      </div>

      <h4 className="mt-3 text-sm font-semibold text-content">{vendor.name}</h4>
      {descKey && <p className="mt-0.5 text-xs leading-relaxed text-content-muted">{t(descKey)}</p>}

      <div className="mt-3 flex flex-wrap gap-1.5">
        {hasHosted ? (
          <IncludedTag />
        ) : (
          <Badge dot={false}>{t('connections.voiceAgents.ownKeyTag')}</Badge>
        )}
      </div>

      <div className="mt-auto flex items-center justify-end gap-2 pt-4">
        {!inUse && ready && (
          <Button
            size="sm"
            variant="secondary"
            analyticsId="live-voice-use-vendor"
            data-testid={`live-voice-use-vendor-${vendor.id}`}
            disabled={saving}
            onClick={() => onUse(ready.id)}>
            {t('connections.voiceAgents.use')}
          </Button>
        )}
        <Button
          size="sm"
          variant="tertiary"
          analyticsId="live-voice-open-settings"
          data-testid={`live-voice-settings-${vendor.id}`}
          aria-label={t('connections.voiceAgents.settingsAria').replace('{name}', vendor.name)}
          onClick={onOpenSettings}>
          <Settings2 className="h-4 w-4" aria-hidden />
          {t('connections.voiceAgents.settings')}
        </Button>
      </div>
    </div>
  );
};

export default LiveVoiceVendorCard;
