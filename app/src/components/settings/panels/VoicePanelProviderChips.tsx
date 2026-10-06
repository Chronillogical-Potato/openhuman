import { AudioLines, AudioWaveform, Cloud, Laptop, type LucideIcon, Sparkles } from 'lucide-react';
import type { ReactNode } from 'react';

import type { VoiceSettings } from '../../../services/api/voiceSettingsApi';
import { Badge, Card, Switch, Tile, TileGrid } from '../../ui';

/** Built-in voice provider slugs with display metadata. */
export const BUILTIN_VOICE_PROVIDER_META: Record<
  string,
  { label: string; capability: 'stt' | 'tts' | 'both'; comingSoon?: boolean }
> = {
  deepgram: { label: 'Deepgram', capability: 'stt', comingSoon: true },
  elevenlabs: { label: 'ElevenLabs', capability: 'both' },
  openai: { label: 'OpenAI', capability: 'both', comingSoon: true },
};

const PROVIDER_ICON: Record<string, LucideIcon> = {
  deepgram: AudioLines,
  elevenlabs: AudioWaveform,
  openai: Sparkles,
};

interface VoicePanelProviderChipsProps {
  t: (key: string) => string;
  sttProvider: string;
  ttsProvider: string;
  onSttProviderChange: (next: string) => void;
  onTtsProviderChange: (next: string) => void;
  voiceSettings: VoiceSettings | null;
  isSavingPendingKey: boolean;
  setPendingKeySlug: (slug: string | null) => void;
  setPendingKeyValue: (value: string) => void;
  handleRemoveProvider: (slug: string) => void | Promise<void>;
}

interface ProviderRowProps {
  icon: LucideIcon;
  label: string;
  capability: 'stt' | 'tts' | 'both';
  enabled: boolean;
  /** Extra chip after the capability chips (e.g. "Always on", "coming soon"). */
  status?: ReactNode;
  dimmed?: boolean;
  t: (key: string) => string;
  children: ReactNode;
}

/** One provider tile: icon, name + what it does, and its enable switch. */
const ProviderRow = ({
  icon: Icon,
  label,
  capability,
  enabled,
  status,
  dimmed,
  t,
  children,
}: ProviderRowProps) => (
  <Tile icon={<Icon />} iconActive={enabled} muted={dimmed} title={label} control={children}>
    <div className="flex flex-wrap items-center gap-1.5">
      {capability !== 'tts' && <Badge variant="neutral">{t('voice.providers.cap.stt')}</Badge>}
      {capability !== 'stt' && <Badge variant="neutral">{t('voice.providers.cap.tts')}</Badge>}
      {status}
    </div>
  </Tile>
);

/**
 * Voice providers as a tile grid: managed cloud (locked on), Piper (local TTS, no
 * key required), and the external BYOK providers. Turning an external
 * provider on opens the key modal; Piper opens its setup modal.
 */
const VoicePanelProviderChips = ({
  t,
  sttProvider,
  ttsProvider,
  onSttProviderChange,
  onTtsProviderChange,
  voiceSettings,
  isSavingPendingKey,
  setPendingKeySlug,
  setPendingKeyValue,
  handleRemoveProvider,
}: VoicePanelProviderChipsProps) => {
  const piperEnabled = ttsProvider === 'piper';

  return (
    <Card
      title={t('voice.providers.title')}
      description={t('voice.providers.cardDesc')}
      divided={false}
      className="border-line-strong"
      data-testid="voice-providers-section">
      <TileGrid padded columns={2}>
        {/* Cloud — always enabled, locked */}
        <ProviderRow
          icon={Cloud}
          label={t('voice.providers.chip.cloud')}
          capability="both"
          enabled
          status={<Badge variant="success">{t('voice.providers.alwaysOn')}</Badge>}
          t={t}>
          <Switch
            id="voice-provider-chip-cloud"
            checked
            disabled
            onCheckedChange={() => {}}
            aria-label={t('voice.providers.chip.cloudAria')}
          />
        </ProviderRow>

        {/* Piper — local TTS, no API key required. Turning it on opens the
          setup modal (the user supplies the piper binary and voice; Enable
          then calls voice_update_provider_settings). Turning it off routes TTS
          back to the managed cloud provider. */}
        <ProviderRow
          icon={Laptop}
          label={t('voice.providers.chip.piper')}
          capability="tts"
          enabled={piperEnabled}
          t={t}>
          <Switch
            id="voice-provider-chip-piper"
            data-testid="voice-provider-chip-piper"
            checked={piperEnabled}
            onCheckedChange={next => {
              if (!next) {
                onTtsProviderChange('cloud');
              } else {
                setPendingKeySlug('piper');
                setPendingKeyValue('');
              }
            }}
            aria-label={
              piperEnabled
                ? `${t('voice.providers.chip.disableProvider')} ${t('voice.providers.chip.piper')}`
                : `${t('voice.providers.chip.enableProvider')} ${t('voice.providers.chip.piper')}`
            }
          />
        </ProviderRow>

        {/* External providers: Deepgram, ElevenLabs, OpenAI */}
        {Object.entries(BUILTIN_VOICE_PROVIDER_META).map(([slug, meta]) => {
          const enabled = (voiceSettings?.voiceProviders ?? []).some(p => p.slug === slug);
          return (
            <ProviderRow
              key={slug}
              icon={PROVIDER_ICON[slug] ?? AudioLines}
              label={meta.label}
              capability={meta.capability}
              enabled={enabled}
              dimmed={meta.comingSoon}
              status={
                meta.comingSoon ? (
                  <Badge variant="neutral">{t('voice.providers.chip.comingSoon')}</Badge>
                ) : undefined
              }
              t={t}>
              <Switch
                id={`voice-provider-chip-${slug}`}
                data-testid={`voice-provider-chip-${slug}`}
                checked={enabled}
                disabled={isSavingPendingKey || !!meta.comingSoon}
                onCheckedChange={next => {
                  if (meta.comingSoon) return;
                  if (!next) {
                    void handleRemoveProvider(slug);
                    if (sttProvider === slug) onSttProviderChange('cloud');
                    if (ttsProvider === slug) onTtsProviderChange('cloud');
                  } else {
                    setPendingKeySlug(slug);
                    setPendingKeyValue('');
                  }
                }}
                aria-label={
                  enabled
                    ? `${t('voice.providers.chip.disableProvider')} ${meta.label}`
                    : `${t('voice.providers.chip.enableProvider')} ${meta.label}`
                }
              />
            </ProviderRow>
          );
        })}
      </TileGrid>
    </Card>
  );
};

export default VoicePanelProviderChips;
