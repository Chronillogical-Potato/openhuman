import { AudioWaveform, KeyRound, Languages, type LucideIcon, Sparkles } from 'lucide-react';
import { useState } from 'react';

import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import type {
  LiveVoiceProvider,
  LiveVoiceSettings,
  LiveVoiceSettingsPatch,
  LiveVoiceTestResult,
} from '../../../services/api/liveVoiceApi';
import Badge from '../../ui/Badge';
import Button from '../../ui/Button';
import NativeSelect from '../../ui/NativeSelect';
import TextField from '../../ui/TextField';

export type LiveVoiceTestState =
  | { kind: 'testing' }
  | { kind: 'done'; result: LiveVoiceTestResult };

const PROVIDER_ICON: Record<string, LucideIcon> = {
  'gemini-hosted': Sparkles,
  'elevenlabs-hosted': AudioWaveform,
  gemini: KeyRound,
  sarvam: Languages,
};

/** Description key per known provider (explicit so the i18n audit sees them). */
const PROVIDER_DESC_KEY: Record<string, string> = {
  'gemini-hosted': 'connections.voiceAgents.descGeminiHosted',
  'elevenlabs-hosted': 'connections.voiceAgents.descElevenlabsHosted',
  gemini: 'connections.voiceAgents.descGemini',
  sarvam: 'connections.voiceAgents.descSarvam',
};

/**
 * Which settings block a provider's voice/language pickers write to, and the
 * field names inside it. Hosted and BYOK Gemini share the `gemini` block.
 */
function voiceFields(
  providerId: string
):
  | { block: 'gemini'; voice: 'voice'; language: 'language' }
  | { block: 'sarvam'; voice: 'speaker'; language: 'language' }
  | { block: 'elevenlabs'; voice: 'voice_id'; language: null }
  | null {
  if (providerId === 'gemini' || providerId === 'gemini-hosted') {
    return { block: 'gemini', voice: 'voice', language: 'language' };
  }
  if (providerId === 'sarvam') return { block: 'sarvam', voice: 'speaker', language: 'language' };
  if (providerId === 'elevenlabs-hosted') {
    return { block: 'elevenlabs', voice: 'voice_id', language: null };
  }
  return null;
}

function readSetting(settings: LiveVoiceSettings, block: string, field: string): string {
  const values = (settings as unknown as Record<string, Record<string, string | null>>)[block];
  return values?.[field] ?? '';
}

/** Inset that lines card sections up with the title, past the radio and icon. */
const BODY_INSET = 'pl-4 sm:pl-[98px]';

export interface LiveVoiceProviderCardProps {
  provider: LiveVoiceProvider;
  /** Radio group name shared by every card on the page. */
  groupName: string;
  selected: boolean;
  settings: LiveVoiceSettings | null;
  saving: boolean;
  test: LiveVoiceTestState | undefined;
  keyDraft: string;
  onSelect: () => void;
  onTest: () => void;
  onKeyDraft: (value: string) => void;
  onSaveKey: () => void;
  onClearKey: () => void;
  onPersist: (patch: LiveVoiceSettingsPatch) => void;
}

/**
 * One voice agent on Connections → Voice agents: a radio row that makes it the
 * agent the mascot talks through, a connectivity test, the BYOK key editor,
 * and — on the agent in use — its voice and language pickers.
 */
const LiveVoiceProviderCard = ({
  provider,
  groupName,
  selected,
  settings,
  saving,
  test,
  keyDraft,
  onSelect,
  onTest,
  onKeyDraft,
  onSaveKey,
  onClearKey,
  onPersist,
}: LiveVoiceProviderCardProps) => {
  const { t } = useT();
  const [replacingKey, setReplacingKey] = useState(false);
  const Icon = PROVIDER_ICON[provider.id] ?? Sparkles;
  const testing = test?.kind === 'testing';
  const selectable = provider.configured && !saving;
  const radioId = `live-voice-radio-${provider.id}`;
  const descKey = PROVIDER_DESC_KEY[provider.id];

  const statusBadge = !provider.configured ? (
    <Badge variant="warning">{t('connections.voiceAgents.badgeNeedsKey')}</Badge>
  ) : test?.kind === 'done' && !test.result.ok ? (
    <Badge variant="danger">{t('connections.voiceAgents.badgeFailed')}</Badge>
  ) : (
    <Badge variant="success">{t('connections.voiceAgents.badgeReady')}</Badge>
  );

  const testLine = () => {
    if (!test) return null;
    if (test.kind === 'testing') {
      return (
        <p className="text-xs text-content-muted" data-testid={`live-voice-test-${provider.id}`}>
          {t('connections.voiceAgents.testing')}
        </p>
      );
    }
    const { result } = test;
    return (
      <p
        className={cn(
          'text-xs',
          result.ok ? 'text-sage-700 dark:text-sage-300' : 'break-words text-coral-600'
        )}
        data-testid={`live-voice-test-${provider.id}`}
        data-ok={result.ok ? 'true' : 'false'}>
        {result.ok
          ? result.latency_ms != null
            ? t('connections.voiceAgents.testOk').replace('{ms}', String(result.latency_ms))
            : t('connections.voiceAgents.testOkNoLatency')
          : t('connections.voiceAgents.testFailed').replace('{error}', result.error ?? '')}
      </p>
    );
  };

  const pickers = () => {
    if (!settings || !selected) return null;
    const fields = voiceFields(provider.id);
    if (!fields) return null;
    const showVoice = provider.voices.length > 0;
    const showLanguage = fields.language != null && provider.languages.length > 0;
    if (!showVoice && !showLanguage) return null;
    const voiceLabel =
      fields.voice === 'speaker'
        ? t('connections.voiceAgents.speaker')
        : t('connections.voiceAgents.voice');
    return (
      <div className="grid gap-3 sm:grid-cols-2">
        {showVoice && (
          <label className="flex flex-col gap-1.5 text-xs font-medium text-content-secondary">
            <span>{voiceLabel}</span>
            <NativeSelect
              inputSize="sm"
              aria-label={`${provider.label} ${voiceLabel}`}
              data-testid={`live-voice-voice-${provider.id}`}
              value={readSetting(settings, fields.block, fields.voice)}
              disabled={saving}
              onChange={e =>
                onPersist({ [fields.block]: { [fields.voice]: e.target.value || null } })
              }>
              <option value="">{t('connections.voiceAgents.providerDefault')}</option>
              {provider.voices.map(v => (
                <option key={v} value={v}>
                  {v}
                </option>
              ))}
            </NativeSelect>
          </label>
        )}
        {showLanguage && fields.language && (
          <label className="flex flex-col gap-1.5 text-xs font-medium text-content-secondary">
            <span>{t('connections.voiceAgents.language')}</span>
            <NativeSelect
              inputSize="sm"
              aria-label={`${provider.label} ${t('connections.voiceAgents.language')}`}
              data-testid={`live-voice-language-${provider.id}`}
              value={readSetting(settings, fields.block, fields.language)}
              disabled={saving}
              onChange={e =>
                onPersist({
                  [fields.block]: { [fields.language as string]: e.target.value || null },
                })
              }>
              <option value="">{t('connections.voiceAgents.providerDefault')}</option>
              {provider.languages.map(l => (
                <option key={l} value={l}>
                  {l}
                </option>
              ))}
            </NativeSelect>
          </label>
        )}
      </div>
    );
  };

  const keyInput = (
    <div className="flex flex-wrap items-center gap-2">
      <TextField
        type="password"
        autoComplete="off"
        inputSize="sm"
        className="min-w-0 flex-1"
        aria-label={`${provider.label} ${t('connections.voiceAgents.apiKey')}`}
        placeholder={t('connections.voiceAgents.apiKeyPlaceholder')}
        data-testid={`live-voice-key-${provider.id}`}
        value={keyDraft}
        disabled={saving}
        onChange={e => onKeyDraft(e.target.value)}
      />
      <Button
        size="sm"
        analyticsId="live-voice-save-key"
        data-testid={`live-voice-save-key-${provider.id}`}
        disabled={saving || !keyDraft.trim()}
        onClick={() => {
          onSaveKey();
          setReplacingKey(false);
        }}>
        {t('connections.voiceAgents.saveKey')}
      </Button>
    </div>
  );

  const keyEditor = () => {
    if (provider.kind !== 'byok' || !provider.key_slug) return null;
    if (!provider.configured) {
      return (
        <div className="flex flex-col gap-2">
          <p className="text-xs text-content-muted">{t('connections.voiceAgents.addKeyHint')}</p>
          {keyInput}
        </div>
      );
    }
    return (
      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <span className="flex min-w-0 flex-1 items-center gap-1.5 text-xs text-content-secondary">
            <KeyRound className="h-3.5 w-3.5 shrink-0" aria-hidden />
            {t('connections.voiceAgents.keyOnFile')}
          </span>
          <Button
            size="xs"
            variant="tertiary"
            analyticsId="live-voice-replace-key"
            data-testid={`live-voice-replace-key-${provider.id}`}
            disabled={saving}
            aria-expanded={replacingKey}
            onClick={() => setReplacingKey(v => !v)}>
            {replacingKey ? t('common.cancel') : t('connections.voiceAgents.replaceKey')}
          </Button>
          <Button
            size="xs"
            variant="tertiary"
            tone="danger"
            analyticsId="live-voice-clear-key"
            data-testid={`live-voice-clear-key-${provider.id}`}
            disabled={saving}
            onClick={onClearKey}>
            {t('connections.voiceAgents.clearKey')}
          </Button>
        </div>
        {replacingKey && keyInput}
      </div>
    );
  };

  const pickerBody = pickers();
  const keyBody = keyEditor();
  const testBody = testLine();

  return (
    <div
      data-testid={`live-voice-provider-${provider.id}`}
      data-selected={selected || undefined}
      className={cn(
        'overflow-hidden rounded-xl border transition-colors',
        selected
          ? 'border-primary-500 bg-primary-50 ring-1 ring-primary-500 dark:bg-primary-500/10'
          : 'border-line bg-surface',
        !provider.configured && 'border-dashed'
      )}>
      <div className="flex items-center gap-3 px-4 py-3.5">
        <label
          htmlFor={radioId}
          className={cn(
            'group flex min-w-0 flex-1 items-center gap-3',
            selectable ? 'cursor-pointer' : 'cursor-default'
          )}>
          <input
            id={radioId}
            type="radio"
            name={groupName}
            value={provider.id}
            className="peer sr-only"
            checked={selected}
            disabled={!provider.configured || saving}
            data-testid={`live-voice-default-${provider.id}`}
            onChange={onSelect}
          />
          <span
            aria-hidden
            className={cn(
              'flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded-full border-2 transition-colors',
              'peer-focus-visible:ring-2 peer-focus-visible:ring-primary-500 peer-focus-visible:ring-offset-2',
              selected
                ? 'border-primary-500'
                : provider.configured
                  ? 'border-line-strong group-hover:border-primary-400'
                  : 'border-line'
            )}>
            {selected && <span className="h-2 w-2 rounded-full bg-primary-500" />}
          </span>
          <span
            aria-hidden
            className={cn(
              'flex h-10 w-10 shrink-0 items-center justify-center rounded-xl [&_svg]:h-5 [&_svg]:w-5',
              selected
                ? 'bg-primary-500 text-content-inverted'
                : provider.configured
                  ? 'bg-surface-strong text-content'
                  : 'bg-surface-muted text-content-faint'
            )}>
            <Icon />
          </span>
          <span className="min-w-0">
            <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
              <span className="text-sm font-semibold text-content">{provider.label}</span>
              {selected && (
                <Badge variant="primary">{t('connections.voiceAgents.badgeInUse')}</Badge>
              )}
            </span>
            {descKey && (
              <span className="mt-0.5 block text-xs leading-relaxed text-content-muted">
                {t(descKey)}
              </span>
            )}
          </span>
        </label>
        <div className="flex shrink-0 items-center gap-2">
          <span className="hidden sm:inline-flex">{statusBadge}</span>
          {provider.configured && (
            <Button
              size="sm"
              variant="secondary"
              analyticsId="live-voice-test-provider"
              data-testid={`live-voice-test-button-${provider.id}`}
              disabled={testing}
              onClick={onTest}>
              {testing ? t('connections.voiceAgents.testing') : t('connections.voiceAgents.test')}
            </Button>
          )}
        </div>
      </div>
      {testBody && <div className={cn('-mt-1.5 pr-4 pb-3', BODY_INSET)}>{testBody}</div>}
      {(pickerBody || keyBody) && (
        <div
          className={cn(
            'flex flex-col gap-3 border-t py-3 pr-4',
            BODY_INSET,
            selected ? 'border-primary-500/20' : 'border-line-subtle'
          )}>
          {pickerBody}
          {keyBody}
        </div>
      )}
    </div>
  );
};

export default LiveVoiceProviderCard;
