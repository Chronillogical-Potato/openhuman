// [composio-direct] Settings panel for the Composio routing mode toggle
// (Backend / Direct BYO API key). Shipped in PR3 of #1710 — see
// `crates/openhuman-core/src/integrations/composio/client.rs::create_composio_client` for the
// matching Rust factory.
//
// Why a separate panel from ComposioTriagePanel:
//   - ComposioTriagePanel governs the per-trigger LLM triage opt-out,
//     a behavior that lives entirely inside the backend-proxied
//     pipeline. Mixing the BYO-key controls into it would conflate two
//     orthogonal concerns and confuse users (triggers don't work at all
//     in direct mode — separately calling that out is cleaner).
//   - The BYO-key controls are their own concern, so they live in a new
//     file rather than being folded into an existing provider panel.
import { Cloud, KeyRound, type LucideIcon, Save } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '../../../lib/cn';
import { useT } from '../../../lib/i18n/I18nContext';
import { useCoreState } from '../../../providers/CoreStateProvider';
import { isLocalSessionToken } from '../../../utils/localSession';
import {
  type ComposioModeStatus,
  openhumanComposioClearApiKey,
  openhumanComposioGetMode,
  openhumanComposioSetApiKey,
} from '../../../utils/tauriCommands';
import PanelPage from '../../layout/PanelPage';
import Alert, { AlertDescription, AlertTitle } from '../../ui/Alert';
import {
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogRoot,
  AlertDialogTitle,
} from '../../ui/AlertDialog';
import Badge from '../../ui/Badge';
import Button from '../../ui/Button';
import Card from '../../ui/Card';
import Field from '../../ui/Field';
import { Spinner } from '../../ui/icons';
import { CenteredLoadingState } from '../../ui/LoadingState';
import { RadioGroupItem, RadioGroupRoot } from '../../ui/RadioGroup';
import StatusLine from '../../ui/StatusLine';
import TextField from '../../ui/TextField';
import SettingsBackButton from '../components/SettingsBackButton';
import { useSettingsNavigation } from '../hooks/useSettingsNavigation';
import ComposioTriagePanel from './ComposioTriagePanel';

type Mode = 'backend' | 'direct';

interface ComposioPanelProps {
  /** When true, render without the SettingsHeader chrome (used when embedded
   *  inside the onboarding custom wizard). */
  embedded?: boolean;
  /** Whether OpenHuman-managed auth should be offered. Defaults to true for
   *  cloud-authenticated sessions and false otherwise. */
  managedAuthEnabled?: boolean;
}

const ComposioPanel = ({ embedded = false, managedAuthEnabled }: ComposioPanelProps = {}) => {
  const { t } = useT();
  const { navigateBack } = useSettingsNavigation();
  const { snapshot } = useCoreState();
  const allowManagedAuth =
    managedAuthEnabled ??
    (Boolean(snapshot.sessionToken) && !isLocalSessionToken(snapshot.sessionToken));

  const [mode, setMode] = useState<Mode>('backend');
  // Tracks the mode that's actually persisted on disk — distinct from
  // the in-flight `mode` radio selection so we can tell whether a Save
  // click constitutes a Backend → Direct *transition* (which needs a
  // confirmation gate) vs. just persisting a new API key while already
  // in Direct mode.
  const [persistedMode, setPersistedMode] = useState<Mode>('backend');
  const [apiKey, setApiKey] = useState('');
  const [apiKeyStored, setApiKeyStored] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saved' | 'error' | 'cleared'>('idle');
  const [saveError, setSaveError] = useState<string | null>(null);
  // Confirmation gate for the Backend → Direct transition. The state
  // machine has two arms: `idle` (Save acts immediately) and
  // `awaiting` (Save was clicked while transitioning Backend → Direct
  // with a fresh key — the user sees the warning copy and must hit
  // "I understand, switch to Direct" or "Cancel"). Direct → Backend
  // doesn't need this because that recovery is reversible (re-paste
  // the key to flip back).
  const [confirmGate, setConfirmGate] = useState<'idle' | 'awaiting'>('idle');
  const saveStatusTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── load current mode status on mount ────────────────────────────
  useEffect(() => {
    let isMounted = true;
    openhumanComposioGetMode()
      .then(res => {
        if (!isMounted) return;
        const status: ComposioModeStatus | undefined = res.result;
        if (!status) return;
        const normalizedMode: Mode =
          !allowManagedAuth || status.mode === 'direct' ? 'direct' : 'backend';
        setMode(normalizedMode);
        setPersistedMode(normalizedMode);
        setApiKeyStored(Boolean(status.api_key_set));
      })
      .catch(err => {
        if (!isMounted) return;
        // [composio-direct] never re-throw — settings panel should
        // still render so the user can recover by toggling manually.
        console.warn('[ComposioPanel] failed to load mode:', err);
      })
      .finally(() => {
        if (isMounted) setLoading(false);
      });

    return () => {
      isMounted = false;
      if (saveStatusTimer.current !== null) {
        clearTimeout(saveStatusTimer.current);
      }
    };
  }, [allowManagedAuth]);

  const flashSaved = (status: 'saved' | 'cleared') => {
    setSaveError(null);
    setSaveStatus(status);
    if (saveStatusTimer.current !== null) {
      clearTimeout(saveStatusTimer.current);
    }
    saveStatusTimer.current = setTimeout(() => setSaveStatus('idle'), 3000);
    // [composio-cache] Notify in-renderer subscribers (notably
    // useComposioIntegrations) that the routing config just changed so
    // they can drop their cached connection / toolkit state and
    // re-fetch against the new client. Mirrors the core-side
    // DomainEvent::ComposioConfigChanged emitted by the matching RPC
    // op. Without this the integrations panel keeps showing the
    // previous tenant's badge for up to one poll interval (5s).
    try {
      window.dispatchEvent(new CustomEvent('composio:config-changed'));
    } catch (err) {
      // Non-fatal — old browsers without CustomEvent support shouldn't
      // crash the panel.
      console.warn('[composio-cache] dispatch composio:config-changed failed:', err);
    }
  };

  // Indicates this Save click would transition the persisted mode from
  // Backend to Direct *with* a freshly-pasted key. We gate this exact
  // transition on a confirmation step because the consequences are not
  // obvious from the radio toggle alone — the user's previously-linked
  // integrations (Gmail, Slack, GitHub, …) live in TinyHumans' Composio
  // tenant and will simply disappear from the integrations panel until
  // they re-link them through their personal app.composio.dev account.
  const isBackendToDirectTransition = (): boolean => {
    const trimmed = apiKey.trim();
    return persistedMode === 'backend' && mode === 'direct' && trimmed.length > 0;
  };

  const performSave = async () => {
    const trimmed = apiKey.trim();
    setSaving(true);
    setSaveError(null);
    setSaveStatus('idle');
    try {
      if (mode === 'direct' && trimmed.length > 0) {
        // [composio-direct] persist new key + flip mode to direct.
        await openhumanComposioSetApiKey(trimmed, true);
        // Mask the field after a successful save so the secret is not
        // left dangling in the DOM. The Rust side has the source of
        // truth in the encrypted keychain.
        setApiKey('');
        setApiKeyStored(true);
        setPersistedMode('direct');
        flashSaved('saved');
      } else if (mode === 'backend') {
        // Switching to backend — clear the stored key and reset mode.
        await openhumanComposioClearApiKey();
        setApiKey('');
        setApiKeyStored(false);
        setPersistedMode('backend');
        flashSaved('cleared');
      } else {
        // Direct selected, no new key but one already stored — nothing
        // to persist; just acknowledge.
        flashSaved('saved');
      }
    } catch (err) {
      console.warn('[ComposioPanel] failed to save:', err);
      if (saveStatusTimer.current !== null) {
        clearTimeout(saveStatusTimer.current);
        saveStatusTimer.current = null;
      }
      const errorMessage = err instanceof Error ? err.message.toLowerCase() : '';
      const message = errorMessage.includes('invalid composio api key')
        ? t('settings.composio.invalidApiKey')
        : t('composio.saveFailed');
      setSaveError(message);
      setSaveStatus('error');
    } finally {
      setSaving(false);
      setConfirmGate('idle');
    }
  };

  const handleSave = async () => {
    const trimmed = apiKey.trim();
    if (mode === 'direct' && trimmed.length === 0 && !apiKeyStored) {
      // Direct mode without a key is a no-op — flag it clearly instead
      // of round-tripping to the backend just to get an error string.
      setSaveError(t('settings.composio.saveErrorNoKey'));
      setSaveStatus('error');
      return;
    }
    if (isBackendToDirectTransition()) {
      // [composio-direct] Show the confirmation step instead of saving
      // straight away. The user-visible consequences (existing
      // integrations disappear, triggers don't fire) aren't obvious
      // from the radio toggle alone.
      console.debug('[composio-direct] Backend → Direct transition pending user confirmation');
      setConfirmGate('awaiting');
      return;
    }
    await performSave();
  };

  const handleConfirmTransition = async () => {
    console.debug('[composio-direct] Backend → Direct transition confirmed by user');
    await performSave();
  };

  const handleCancelTransition = () => {
    console.debug('[composio-direct] Backend → Direct transition cancelled by user');
    setConfirmGate('idle');
  };

  const composioDescription = embedded
    ? undefined
    : t('settings.developerMenu.composioRouting.desc');
  const composioLeading = embedded ? undefined : <SettingsBackButton onBack={navigateBack} />;

  if (loading) {
    return (
      <PanelPage contentClassName="" description={composioDescription} leading={composioLeading}>
        <div className={embedded ? '' : 'p-4'}>
          <CenteredLoadingState label={t('settings.composio.loading')} />
        </div>
      </PanelPage>
    );
  }

  const savedNote =
    saveStatus === 'saved'
      ? t('composio.settingsSaved')
      : saveStatus === 'cleared'
        ? t('settings.composio.clearedToBackend')
        : null;

  const MODES: { value: Mode; labelKey: string; descKey: string; icon: LucideIcon }[] = [
    {
      value: 'backend',
      labelKey: 'settings.composio.modeManaged',
      descKey: 'settings.composio.modeManagedDesc',
      icon: Cloud,
    },
    {
      value: 'direct',
      labelKey: 'settings.composio.modeDirect',
      descKey: 'settings.composio.modeDirectDesc',
      icon: KeyRound,
    },
  ];

  // What is actually in effect right now (not the in-flight radio choice).
  const statusBadge =
    persistedMode === 'direct' ? (
      apiKeyStored ? (
        <Badge variant="success">{t('settings.composio.statusDirect')}</Badge>
      ) : (
        <Badge variant="warning">{t('settings.composio.statusNoKey')}</Badge>
      )
    ) : (
      <Badge variant="primary">{t('settings.composio.statusManaged')}</Badge>
    );

  return (
    <PanelPage
      className="z-10"
      contentClassName=""
      description={composioDescription}
      leading={composioLeading}>
      <div className={embedded ? 'space-y-5' : 'space-y-5 p-4 pt-2'}>
        {!allowManagedAuth && (
          <Alert variant="info">
            <div>
              <AlertTitle>{t('settings.composio.modeDirect')}</AlertTitle>
              <AlertDescription>
                {t(
                  'settings.composio.directOnlyDesc',
                  'Managed Composio auth is unavailable here. Enter your own Composio API key or skip this for now.'
                )}
              </AlertDescription>
            </div>
          </Alert>
        )}

        {/* ── Routing: managed vs. bring-your-own key, the key itself, and
            the save footer, all in one card. ───────────────────────────── */}
        <Card
          title={t('settings.composio.routingMode')}
          description={t('settings.composio.intro')}
          headerRight={statusBadge}
          data-testid="composio-routing-card">
          {allowManagedAuth && (
            <div className="p-4">
              <RadioGroupRoot
                value={mode}
                onValueChange={value => setMode(value as Mode)}
                aria-label={t('settings.composio.routingMode')}
                className="grid gap-2 md:grid-cols-2">
                {MODES.map(({ value, labelKey, descKey, icon: Icon }) => {
                  const selected = mode === value;
                  const inputId = `composio-mode-${value}`;
                  return (
                    <label
                      key={value}
                      htmlFor={inputId}
                      className={cn(
                        'flex cursor-pointer items-start gap-3 rounded-xl border px-3.5 py-3 transition-colors',
                        selected
                          ? 'border-primary-500 bg-primary-50 ring-1 ring-primary-500 dark:bg-primary-500/10'
                          : 'border-line bg-surface hover:border-line-strong hover:bg-surface-hover'
                      )}>
                      <span
                        className={cn(
                          'flex h-9 w-9 shrink-0 items-center justify-center rounded-lg',
                          selected
                            ? 'bg-primary-500 text-content-inverted'
                            : 'bg-surface-muted text-content-secondary'
                        )}>
                        <Icon className="h-4.5 w-4.5" aria-hidden />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block text-sm font-semibold text-content">
                          {t(labelKey)}
                        </span>
                        <span className="mt-0.5 block text-xs leading-relaxed text-content-muted">
                          {t(descKey)}
                        </span>
                      </span>
                      <RadioGroupItem
                        id={inputId}
                        value={value}
                        aria-label={t(labelKey)}
                        className="mt-1 shrink-0"
                      />
                    </label>
                  );
                })}
              </RadioGroupRoot>
            </div>
          )}

          {/* API key — only when Direct is selected */}
          {mode === 'direct' && (
            <Field
              htmlFor="composio-api-key"
              label={t('settings.composio.apiKeyLabel')}
              description={apiKeyStored ? t('settings.composio.apiKeyDesc') : undefined}
              control={
                <div className="flex items-center gap-2">
                  {apiKeyStored && (
                    <Badge variant="success">{t('settings.composio.apiKeyStored')}</Badge>
                  )}
                  <TextField
                    id="composio-api-key"
                    type="password"
                    autoComplete="off"
                    inputSize="sm"
                    className="w-72"
                    value={apiKey}
                    onChange={e => setApiKey(e.target.value)}
                    placeholder={
                      apiKeyStored
                        ? t('settings.composio.apiKeyStoredPlaceholder')
                        : t('settings.composio.apiKeyExamplePlaceholder')
                    }
                    aria-label={t('settings.composio.apiKeyLabel')}
                    mono
                  />
                </div>
              }
            />
          )}

          <div className="flex items-center justify-between gap-3 px-4 py-3">
            <StatusLine
              saving={false}
              savedNote={savedNote}
              error={saveStatus === 'error' ? (saveError ?? t('composio.saveFailed')) : null}
              savingLabel=""
              className="min-h-0"
            />
            <Button
              type="button"
              variant="primary"
              size="sm"
              leadingIcon={saving ? <Spinner /> : <Save className="h-3.5 w-3.5" aria-hidden />}
              onClick={() => void handleSave()}
              disabled={saving || confirmGate === 'awaiting'}>
              {saving ? t('settings.composio.saving') : t('common.save')}
            </Button>
          </div>
        </Card>

        <AlertDialogRoot
          open={confirmGate === 'awaiting'}
          onOpenChange={open => {
            if (!open) handleCancelTransition();
          }}>
          <AlertDialogContent className="max-w-md">
            <AlertDialogTitle>{t('settings.composio.confirmTitle')}</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 leading-relaxed">
                <p>{t('settings.composio.confirmWarning')}</p>
                <p>{t('settings.composio.confirmNeedItems')}</p>
                <ol className="list-decimal space-y-1 pl-5">
                  <li>{t('settings.composio.confirmItem1')}</li>
                  <li>{t('settings.composio.confirmItem2')}</li>
                  <li>{t('settings.composio.confirmItem3')}</li>
                </ol>
              </div>
            </AlertDialogDescription>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={saving}>{t('common.cancel')}</AlertDialogCancel>
              <AlertDialogAction
                tone="default"
                onClick={() => void handleConfirmTransition()}
                disabled={saving}>
                {saving && <Spinner />}
                {saving ? t('settings.composio.switching') : t('settings.composio.confirmSwitch')}
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialogRoot>

        {/* Integration-trigger triage config (formerly the standalone
            Settings → Developer → Composio triggers page), merged in here so
            all Composio configuration lives on one Connections surface. */}
        <ComposioTriagePanel embedded />
      </div>
    </PanelPage>
  );
};

export default ComposioPanel;
