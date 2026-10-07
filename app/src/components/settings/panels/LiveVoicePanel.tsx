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
  saveLiveVoiceProviderKey,
  testLiveVoiceProvider,
  updateLiveVoiceSettings,
} from '../../../services/api/liveVoiceApi';
import { Alert, AlertDescription } from '../../ui/Alert';
import { CenteredLoadingState } from '../../ui/LoadingState';
import StatusLine from '../../ui/StatusLine';
import SettingsTabbedPage from '../layout/SettingsTabbedPage';
import LiveVoiceProviderCard, { type LiveVoiceTestState } from './LiveVoiceProviderCard';

type Status =
  | { kind: 'idle' }
  | { kind: 'saving' }
  | { kind: 'saved'; message: string }
  | { kind: 'error'; message: string };

const errorMessage = (err: unknown) => (err instanceof Error ? err.message : String(err));

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
  const groupName = useId();
  const [catalog, setCatalog] = useState<LiveVoiceProviders | null>(null);
  const [settings, setSettings] = useState<LiveVoiceSettings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [status, setStatus] = useState<Status>({ kind: 'idle' });
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [tests, setTests] = useState<Record<string, LiveVoiceTestState>>({});
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

  const renderCard = (provider: LiveVoiceProvider) => (
    <LiveVoiceProviderCard
      key={provider.id}
      provider={provider}
      groupName={groupName}
      selected={provider.id === defaultProvider}
      settings={settings}
      saving={saving}
      test={tests[provider.id]}
      keyDraft={keyDrafts[provider.id] ?? ''}
      onSelect={() => void persist({ default_provider: provider.id })}
      onTest={() => void runTest(provider.id)}
      onKeyDraft={value => setKeyDrafts(prev => ({ ...prev, [provider.id]: value }))}
      onSaveKey={() => void saveKey(provider)}
      onClearKey={() => void clearKey(provider)}
      onPersist={patch => void persist(patch)}
    />
  );

  const groups = catalog
    ? [
        {
          id: 'hosted',
          title: t('connections.voiceAgents.groupIncluded'),
          description: t('connections.voiceAgents.groupIncludedDesc'),
          providers: catalog.providers.filter(p => p.kind === 'hosted'),
        },
        {
          id: 'byok',
          title: t('connections.voiceAgents.groupByok'),
          description: t('connections.voiceAgents.groupByokDesc'),
          providers: catalog.providers.filter(p => p.kind !== 'hosted'),
        },
      ].filter(g => g.providers.length > 0)
    : [];

  return (
    <SettingsTabbedPage
      title={t('connections.tabs.voiceAgents')}
      description={t('connections.header.voiceAgents')}>
      <div className="flex w-full max-w-3xl flex-col gap-6" data-testid="live-voice-panel">
        {loadError && (
          <Alert variant="destructive">
            <AlertDescription>
              {t('connections.voiceAgents.loadFailed')}: {loadError}
            </AlertDescription>
          </Alert>
        )}
        {!catalog && !loadError && <CenteredLoadingState label={t('common.loading')} />}

        {catalog && (
          <div
            role="radiogroup"
            aria-label={t('connections.voiceAgents.chooseAgent')}
            className="flex flex-col gap-6"
            data-testid="live-voice-providers">
            {groups.map(group => (
              <section
                key={group.id}
                aria-labelledby={`${groupName}-${group.id}`}
                className="flex flex-col gap-2.5"
                data-testid={`live-voice-group-${group.id}`}>
                <header className="px-1">
                  <h3
                    id={`${groupName}-${group.id}`}
                    className="text-sm font-semibold text-content">
                    {group.title}
                  </h3>
                  <p className="mt-0.5 text-xs text-content-muted">{group.description}</p>
                </header>
                <div className="flex flex-col gap-2.5">{group.providers.map(renderCard)}</div>
              </section>
            ))}
          </div>
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
