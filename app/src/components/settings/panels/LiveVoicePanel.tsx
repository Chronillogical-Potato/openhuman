import { AudioWaveform, KeyRound, Languages, type LucideIcon, Sparkles } from 'lucide-react';
import { useCallback, useEffect, useId, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import {
  clearLiveVoiceProviderKey,
  fetchLiveVoiceProviders,
  fetchLiveVoiceSettings,
  type LiveVoiceProvider,
  type LiveVoiceProviders,
  type LiveVoiceSettings,
  type LiveVoiceSettingsPatch,
  type LiveVoiceTestResult,
  saveLiveVoiceProviderKey,
  testLiveVoiceProvider,
  updateLiveVoiceSettings,
} from '../../../services/api/liveVoiceApi';
import { Alert, AlertDescription } from '../../ui/Alert';
import Badge from '../../ui/Badge';
import Button from '../../ui/Button';
import { CenteredLoadingState } from '../../ui/LoadingState';
import NativeSelect from '../../ui/NativeSelect';
import StatusLine from '../../ui/StatusLine';
import TextField from '../../ui/TextField';
import { Tile, TileGrid } from '../../ui/TileGrid';
import SettingsTabbedPage from '../layout/SettingsTabbedPage';

type Status =
  | { kind: 'idle' }
  | { kind: 'saving' }
  | { kind: 'saved'; message: string }
  | { kind: 'error'; message: string };

type TestState = { kind: 'testing' } | { kind: 'done'; result: LiveVoiceTestResult };

const errorMessage = (err: unknown) => (err instanceof Error ? err.message : String(err));

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

/**
 * Connections → Voice agents: which provider powers the live voice agent (the
 * mascot's talk mode), BYOK keys for the providers that need one, per-provider
 * voice/language choices, and a live connectivity test per provider.
 *
 * Everything is rendered from the core's `voice_live_providers` /
 * `voice_live_settings_get`; every settings write returns the full settings,
 * which replace local state.
 */
const LiveVoicePanel = () => {
  const { t } = useT();
  const defaultSelectId = useId();
  const [catalog, setCatalog] = useState<LiveVoiceProviders | null>(null);
  const [settings, setSettings] = useState<LiveVoiceSettings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [status, setStatus] = useState<Status>({ kind: 'idle' });
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [tests, setTests] = useState<Record<string, TestState>>({});
  const saving = status.kind === 'saving';

  const loadProviders = useCallback(async () => {
    const next = await fetchLiveVoiceProviders();
    setCatalog(next);
    return next;
  }, []);

  useEffect(() => {
    let cancelled = false;
    Promise.all([fetchLiveVoiceProviders(), fetchLiveVoiceSettings()])
      .then(([providers, current]) => {
        if (cancelled) return;
        setCatalog(providers);
        setSettings(current);
      })
      .catch(err => {
        if (!cancelled) setLoadError(errorMessage(err));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const persist = async (patch: LiveVoiceSettingsPatch) => {
    setStatus({ kind: 'saving' });
    try {
      const next = await updateLiveVoiceSettings(patch);
      setSettings(next);
      if (patch.default_provider) {
        setCatalog(prev => (prev ? { ...prev, default_provider: patch.default_provider! } : prev));
      }
      setStatus({ kind: 'saved', message: t('connections.voiceAgents.saved') });
    } catch (err) {
      setStatus({ kind: 'error', message: errorMessage(err) });
    }
  };

  const saveKey = async (provider: LiveVoiceProvider) => {
    const draft = keyDrafts[provider.id]?.trim();
    if (!draft || !provider.key_slug) return;
    setStatus({ kind: 'saving' });
    try {
      await saveLiveVoiceProviderKey(provider.key_slug, draft);
      setKeyDrafts(prev => ({ ...prev, [provider.id]: '' }));
      await loadProviders();
      setStatus({ kind: 'saved', message: t('connections.voiceAgents.keySaved') });
    } catch (err) {
      setStatus({ kind: 'error', message: errorMessage(err) });
    }
  };

  const clearKey = async (provider: LiveVoiceProvider) => {
    if (!provider.key_slug) return;
    setStatus({ kind: 'saving' });
    try {
      await clearLiveVoiceProviderKey(provider.key_slug);
      await loadProviders();
      setStatus({ kind: 'saved', message: t('connections.voiceAgents.keyCleared') });
    } catch (err) {
      setStatus({ kind: 'error', message: errorMessage(err) });
    }
  };

  const runTest = async (providerId: string) => {
    setTests(prev => ({ ...prev, [providerId]: { kind: 'testing' } }));
    try {
      const result = await testLiveVoiceProvider(providerId);
      setTests(prev => ({ ...prev, [providerId]: { kind: 'done', result } }));
    } catch (err) {
      setTests(prev => ({
        ...prev,
        [providerId]: {
          kind: 'done',
          result: { ok: false, latency_ms: null, error: errorMessage(err) },
        },
      }));
    }
  };

  const defaultProvider = settings?.default_provider || catalog?.default_provider || '';

  const testLine = (providerId: string) => {
    const state = tests[providerId];
    if (!state) return null;
    if (state.kind === 'testing') {
      return (
        <p className="text-xs text-content-muted" data-testid={`live-voice-test-${providerId}`}>
          {t('connections.voiceAgents.testing')}
        </p>
      );
    }
    const { result } = state;
    return (
      <p
        className={
          result.ok ? 'text-xs text-sage-700 dark:text-sage-300' : 'text-xs text-coral-600'
        }
        data-testid={`live-voice-test-${providerId}`}
        data-ok={result.ok ? 'true' : 'false'}>
        {result.ok
          ? result.latency_ms != null
            ? t('connections.voiceAgents.testOk').replace('{ms}', String(result.latency_ms))
            : t('connections.voiceAgents.testOkNoLatency')
          : t('connections.voiceAgents.testFailed').replace('{error}', result.error ?? '')}
      </p>
    );
  };

  const pickers = (provider: LiveVoiceProvider) => {
    if (!settings) return null;
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
      <div className="mt-2 grid gap-2 sm:grid-cols-2">
        {showVoice && (
          <label className="flex flex-col gap-1 text-xs text-content-secondary">
            <span>{voiceLabel}</span>
            <NativeSelect
              inputSize="sm"
              aria-label={`${provider.label} ${voiceLabel}`}
              data-testid={`live-voice-voice-${provider.id}`}
              value={readSetting(settings, fields.block, fields.voice)}
              disabled={saving}
              onChange={e =>
                void persist({ [fields.block]: { [fields.voice]: e.target.value || null } })
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
          <label className="flex flex-col gap-1 text-xs text-content-secondary">
            <span>{t('connections.voiceAgents.language')}</span>
            <NativeSelect
              inputSize="sm"
              aria-label={`${provider.label} ${t('connections.voiceAgents.language')}`}
              data-testid={`live-voice-language-${provider.id}`}
              value={readSetting(settings, fields.block, fields.language)}
              disabled={saving}
              onChange={e =>
                void persist({
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

  const keyEditor = (provider: LiveVoiceProvider) => {
    if (provider.kind !== 'byok' || !provider.key_slug) return null;
    const draft = keyDrafts[provider.id] ?? '';
    return (
      <div className="mt-2 flex flex-wrap items-center gap-2">
        <TextField
          type="password"
          autoComplete="off"
          inputSize="sm"
          className="min-w-0 flex-1"
          aria-label={`${provider.label} ${t('connections.voiceAgents.apiKey')}`}
          placeholder={t('connections.voiceAgents.apiKeyPlaceholder')}
          data-testid={`live-voice-key-${provider.id}`}
          value={draft}
          disabled={saving}
          onChange={e => setKeyDrafts(prev => ({ ...prev, [provider.id]: e.target.value }))}
        />
        <Button
          size="sm"
          analyticsId="live-voice-save-key"
          data-testid={`live-voice-save-key-${provider.id}`}
          disabled={saving || !draft.trim()}
          onClick={() => void saveKey(provider)}>
          {t('connections.voiceAgents.saveKey')}
        </Button>
        {provider.configured && (
          <Button
            size="sm"
            variant="tertiary"
            analyticsId="live-voice-clear-key"
            data-testid={`live-voice-clear-key-${provider.id}`}
            disabled={saving}
            onClick={() => void clearKey(provider)}>
            {t('connections.voiceAgents.clearKey')}
          </Button>
        )}
      </div>
    );
  };

  const defaultSelect = catalog ? (
    <label
      htmlFor={defaultSelectId}
      className="flex items-center gap-2.5 rounded-lg border border-line bg-surface px-3 py-1.5 text-sm font-medium text-content">
      <span>{t('connections.voiceAgents.defaultLabel')}</span>
      <NativeSelect
        id={defaultSelectId}
        inputSize="sm"
        data-testid="live-voice-default"
        value={defaultProvider}
        disabled={saving}
        onChange={e => void persist({ default_provider: e.target.value })}>
        {catalog.providers.map(p => (
          <option key={p.id} value={p.id} disabled={!p.configured}>
            {p.configured
              ? p.label
              : t('connections.voiceAgents.needsKeyOption').replace('{label}', p.label)}
          </option>
        ))}
      </NativeSelect>
    </label>
  ) : null;

  return (
    <SettingsTabbedPage
      title={t('connections.tabs.voiceAgents')}
      description={t('connections.header.voiceAgents')}
      headerAction={defaultSelect}>
      <div className="flex w-full flex-col gap-4" data-testid="live-voice-panel">
        {loadError && (
          <Alert variant="destructive">
            <AlertDescription>
              {t('connections.voiceAgents.loadFailed')}: {loadError}
            </AlertDescription>
          </Alert>
        )}
        {!catalog && !loadError && <CenteredLoadingState label={t('common.loading')} />}

        {catalog && (
          <TileGrid columns={2} data-testid="live-voice-providers">
            {catalog.providers.map(provider => {
              const Icon = PROVIDER_ICON[provider.id] ?? Sparkles;
              const isDefault = provider.id === defaultProvider;
              const testing = tests[provider.id]?.kind === 'testing';
              return (
                <Tile
                  key={provider.id}
                  data-testid={`live-voice-provider-${provider.id}`}
                  icon={<Icon />}
                  iconActive={provider.configured}
                  selected={isDefault}
                  title={provider.label}
                  description={
                    PROVIDER_DESC_KEY[provider.id] ? t(PROVIDER_DESC_KEY[provider.id]) : undefined
                  }
                  control={
                    <Button
                      size="sm"
                      variant="secondary"
                      analyticsId="live-voice-test-provider"
                      data-testid={`live-voice-test-button-${provider.id}`}
                      disabled={testing || !provider.configured}
                      onClick={() => void runTest(provider.id)}>
                      {testing
                        ? t('connections.voiceAgents.testing')
                        : t('connections.voiceAgents.test')}
                    </Button>
                  }>
                  <div className="flex flex-wrap items-center gap-1.5">
                    <Badge>
                      {provider.kind === 'hosted'
                        ? t('connections.voiceAgents.kindHosted')
                        : t('connections.voiceAgents.kindByok')}
                    </Badge>
                    {isDefault && (
                      <Badge variant="primary">{t('connections.voiceAgents.badgeDefault')}</Badge>
                    )}
                    <Badge variant={provider.configured ? 'success' : 'warning'}>
                      {provider.configured
                        ? t('connections.voiceAgents.badgeReady')
                        : t('connections.voiceAgents.badgeNeedsKey')}
                    </Badge>
                  </div>
                  {keyEditor(provider)}
                  {pickers(provider)}
                  <div className="mt-1.5">{testLine(provider.id)}</div>
                </Tile>
              );
            })}
          </TileGrid>
        )}

        <StatusLine
          saving={saving}
          savedNote={status.kind === 'saved' ? status.message : null}
          error={
            status.kind === 'error'
              ? t('connections.voiceAgents.saveFailed').replace('{error}', status.message)
              : null
          }
          savingLabel={t('connections.voiceAgents.saving')}
        />
      </div>
    </SettingsTabbedPage>
  );
};

export default LiveVoicePanel;
