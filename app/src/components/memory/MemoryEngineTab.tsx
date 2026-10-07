/**
 * Memory → Provider: where memory is stored. All three providers run on
 * CortexDB, shown as cards (like Connections → Voice agents), each with a
 * Settings modal:
 *
 * - TinyHumans Memory (`builtin`): the `tinyhumans` engine, the TinyHumans
 *   backend's `/memory/*` API (CortexDB hosted per account), authenticated by
 *   sign-in. Included with TinyHumans; Basic and Pro plans ingest for free
 *   under the fair-use terms the modal states. Signed out (or on a local
 *   session) it cannot be selected.
 * - CortexDB Cloud (`apikey`): the `cortexdb` engine on CortexDB's managed
 *   API. The endpoint is fixed; only the key is entered.
 * - CortexDB Local (`selfhost`): the `cortexdb` engine on a server on this
 *   computer. Local is loopback only (a product rule); either scheme is fine
 *   there. The core itself allows https to any host and cleartext http only to
 *   loopback, so this check is the stricter of the two.
 *
 * Which card is active is derived from `memory_engine_get`: `tinyhumans` is
 * TinyHumans, and `cortexdb` is Local when its endpoint is loopback, else
 * Cloud. Engines that are not supported yet are listed as coming soon.
 *
 * debug logging: DEBUG=openhuman:memory:engine
 */
import debug from 'debug';
import { useCallback, useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { useCoreState } from '../../providers/CoreStateProvider';
import {
  type EngineSetRequest,
  type EngineState,
  isMemoryOn,
  memoryEngineSet,
  memoryErrorMessage,
} from '../../services/api/memoryApi';
import { isLocalSessionToken } from '../../utils/localSession';
import { Alert, AlertDescription, AlertTitle, type BadgeVariant } from '../ui';
import { CenteredLoadingState } from '../ui/LoadingState';
import { toast } from '../ui/Toast';
import MemoryComingSoon from './MemoryComingSoon';
import MemoryProviderCard from './MemoryProviderCard';
import MemoryProviderLogo, { type MemoryProviderOption } from './MemoryProviderLogo';
import MemoryProviderModal from './MemoryProviderModal';

export { CORTEXDB_SELF_HOST_DOCS_URL } from './MemoryProviderModal';

const log = debug('openhuman:memory:engine');

type EngineOption = MemoryProviderOption;

const OPTIONS: EngineOption[] = ['builtin', 'apikey', 'selfhost'];

/**
 * True for an http(s) URL whose host is this computer (localhost, 127.x, ::1).
 * Both schemes are accepted: the core refuses only cleartext http off loopback.
 */
export function isLoopbackEndpoint(raw: string): boolean {
  let url: URL;
  try {
    url = new URL(raw.trim());
  } catch {
    return false;
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return false;
  const host = url.hostname;
  return host === 'localhost' || host === '[::1]' || /^127(\.\d{1,3}){3}$/.test(host);
}

/** The provider the configured engine corresponds to. */
function optionOf(state: EngineState | null): EngineOption | null {
  if (state?.engine === 'tinyhumans') return 'builtin';
  if (state?.engine === 'cortexdb') {
    return state.endpoint && isLoopbackEndpoint(state.endpoint) ? 'selfhost' : 'apikey';
  }
  return null;
}

interface MemoryEngineTabProps {
  /** The current engine state (null while the page is still loading it). */
  state: EngineState | null;
  /** Called with the new state after a successful switch. */
  onStateChange: (state: EngineState) => void;
  /** Render without the outer spacing (onboarding embeds this tab). */
  embedded?: boolean;
}

export default function MemoryEngineTab({ state, onStateChange, embedded }: MemoryEngineTabProps) {
  const { t } = useT();
  const { snapshot } = useCoreState();
  const signedIn = snapshot.auth.isAuthenticated && !isLocalSessionToken(snapshot.sessionToken);
  const plan = snapshot.currentUser?.subscription?.plan ?? null;

  const active = optionOf(state);
  const on = isMemoryOn(state);

  const [openOption, setOpenOption] = useState<EngineOption | null>(null);
  const [saving, setSaving] = useState<EngineOption | null>(null);
  const [errors, setErrors] = useState<Partial<Record<EngineOption, string>>>({});
  const [cloudKey, setCloudKey] = useState('');
  // Untouched (null) shows the configured local endpoint, which may arrive
  // after the first render when a host loads the state itself.
  const [typedEndpoint, setTypedEndpoint] = useState<string | null>(null);
  const [localKey, setLocalKey] = useState('');

  const titles: Record<EngineOption, string> = {
    builtin: t('memoryPage.engine.builtin.title'),
    apikey: t('memoryPage.engine.apiKeyOption.title'),
    selfhost: t('memoryPage.engine.selfHost.title'),
  };
  const descriptions: Record<EngineOption, string> = {
    builtin: t('memoryPage.engine.builtin.cardDescription'),
    apikey: t('memoryPage.engine.apiKeyOption.cardDescription'),
    selfhost: t('memoryPage.engine.selfHost.cardDescription'),
  };

  const select = useCallback(
    async (option: EngineOption, req: EngineSetRequest): Promise<boolean> => {
      const wasActive = optionOf(state) === option;
      setSaving(option);
      setErrors(prev => ({ ...prev, [option]: undefined }));
      try {
        const next = await memoryEngineSet(req);
        log('engine set (%s): %s status=%s', option, next.engine ?? 'none', next.status);
        onStateChange(next);
        toast.add(
          wasActive
            ? { type: 'success', title: t('memoryPage.engine.toastSaved') }
            : {
                type: 'success',
                title: t('memoryPage.engine.toastSwitched'),
                description: t('memoryPage.engine.toastSwitchedBody').replace(
                  '{name}',
                  titles[option]
                ),
                data: { icon: <MemoryProviderLogo option={option} className="h-5 w-5" /> },
              }
        );
        setOpenOption(null);
        return true;
      } catch (err) {
        log('engine set (%s) failed: %o', option, err);
        setErrors(prev => ({ ...prev, [option]: memoryErrorMessage(err, t) }));
        return false;
      } finally {
        setSaving(null);
      }
    },
    // `titles` is rebuilt from `t` every render; `t` is the real dependency.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [onStateChange, state, t]
  );

  if (!state) {
    return <CenteredLoadingState label={t('memoryPage.loading')} />;
  }

  // Off: a plain prompt to pick a provider. The core's reason is developer
  // text ("legacy memory backend is unsupported…"), so it is not shown here;
  // degraded and down keep theirs, which names what is wrong.
  const statusBanner = (() => {
    if (!on) {
      return (
        <Alert variant="info" data-testid="memory-engine-status-off">
          <AlertTitle>{t('memoryPage.off.title')}</AlertTitle>
          <AlertDescription>{t('memoryPage.engine.offPrompt')}</AlertDescription>
        </Alert>
      );
    }
    if (state.status === 'degraded' || state.status === 'down') {
      return (
        <Alert
          variant={state.status === 'down' ? 'destructive' : 'warning'}
          data-testid={`memory-engine-status-${state.status}`}>
          <AlertTitle>
            {state.status === 'down'
              ? t('memoryPage.engine.statusDown')
              : t('memoryPage.engine.statusDegraded')}
          </AlertTitle>
          {state.reason ? <AlertDescription>{state.reason}</AlertDescription> : null}
        </Alert>
      );
    }
    return null;
  })();

  const statusOf = (option: EngineOption): { variant: BadgeVariant; label: string } => {
    if (option === active) {
      if (!on) return { variant: 'warning', label: t('memoryPage.engine.statusOff') };
      if (state.status === 'down')
        return { variant: 'danger', label: t('memoryPage.engine.badgeDown') };
      if (state.status === 'degraded') {
        return { variant: 'warning', label: t('memoryPage.engine.badgeDegraded') };
      }
      return { variant: 'primary', label: t('memoryPage.engine.inUse') };
    }
    if (option === 'builtin') {
      return signedIn
        ? { variant: 'success', label: t('memoryPage.engine.badgeReady') }
        : { variant: 'neutral', label: t('memoryPage.engine.builtin.signInRequired') };
    }
    return { variant: 'neutral', label: t('memoryPage.engine.badgeNotConnected') };
  };

  // TinyHumans: one click when signed in.
  const useBuiltin = () => void select('builtin', { engine: 'tinyhumans' });

  // Cloud: the endpoint is CortexDB's managed API. Sending a blank endpoint
  // clears any custom (local) endpoint, so the engine falls back to it.
  const cloudKeyRequired = !(active === 'apikey' && state.has_key);
  const canSubmitCloud = saving === null && (!cloudKeyRequired || cloudKey.trim().length > 0);
  const submitCloud = async () => {
    if (!canSubmitCloud) return;
    const req: EngineSetRequest = { engine: 'cortexdb', endpoint: '' };
    if (cloudKey.trim()) req.api_key = cloudKey.trim();
    if (await select('apikey', req)) setCloudKey('');
  };

  // Local: loopback only.
  const localEndpoint = typedEndpoint ?? (active === 'selfhost' ? (state.endpoint ?? '') : '');
  const endpointTyped = localEndpoint.trim().length > 0;
  const endpointLocal = isLoopbackEndpoint(localEndpoint);
  const localKeyRequired = !(active === 'selfhost' && state.has_key);
  const canSubmitLocal =
    saving === null && endpointLocal && (!localKeyRequired || localKey.trim().length > 0);
  const submitLocal = async () => {
    if (!canSubmitLocal) return;
    const req: EngineSetRequest = { engine: 'cortexdb', endpoint: localEndpoint.trim() };
    if (localKey.trim()) req.api_key = localKey.trim();
    if (await select('selfhost', req)) setLocalKey('');
  };

  const actionOf = (option: EngineOption) => {
    if (option === active && (on || option !== 'builtin')) return undefined;
    if (option === 'builtin') {
      return {
        label:
          saving === 'builtin' ? t('memoryPage.engine.connecting') : t('memoryPage.engine.use'),
        onClick: useBuiltin,
        disabled: saving !== null || !signedIn,
      };
    }
    return { label: t('memoryPage.engine.connect'), onClick: () => setOpenOption(option) };
  };

  const freeNote =
    plan === 'BASIC' || plan === 'PRO'
      ? t('memoryPage.engine.freeIngestion.notePlan').replace(
          '{plan}',
          plan === 'PRO' ? 'Pro' : 'Basic'
        )
      : t('memoryPage.engine.freeIngestion.noteUpgrade');

  return (
    <div
      className={`@container ${embedded ? 'space-y-5' : 'mx-auto w-full max-w-5xl space-y-6 animate-fade-up'}`}
      data-testid="memory-engine-tab">
      {statusBanner}

      <section className="flex flex-col gap-2.5">
        <header>
          <h3 className="text-sm font-semibold text-content">{t('memoryPage.engine.listTitle')}</h3>
          <p className="text-xs text-content-muted">{t('memoryPage.engine.listDescription')}</p>
        </header>
        <div
          className="grid items-stretch gap-3 @xl:grid-cols-2 @4xl:grid-cols-3"
          data-testid="memory-engines">
          {OPTIONS.map(option => (
            <MemoryProviderCard
              key={option}
              option={option}
              title={titles[option]}
              description={descriptions[option]}
              active={option === active}
              status={statusOf(option)}
              action={actionOf(option)}
              note={option === 'builtin' ? freeNote : undefined}
              onOpenSettings={() => setOpenOption(option)}
            />
          ))}
        </div>
      </section>

      {/* Onboarding embeds this tab to pick a provider; upcoming engines are noise there. */}
      {!embedded && <MemoryComingSoon />}

      {openOption && (
        <MemoryProviderModal
          option={openOption}
          title={titles[openOption]}
          description={descriptions[openOption]}
          state={state}
          active={openOption === active}
          signedIn={signedIn}
          plan={plan}
          saving={saving}
          error={errors[openOption]}
          cloudKey={cloudKey}
          onCloudKey={setCloudKey}
          localEndpoint={localEndpoint}
          onLocalEndpoint={setTypedEndpoint}
          endpointInvalid={endpointTyped && !endpointLocal}
          localKey={localKey}
          onLocalKey={setLocalKey}
          canSubmit={
            openOption === 'builtin'
              ? saving === null && signedIn
              : openOption === 'apikey'
                ? canSubmitCloud
                : canSubmitLocal
          }
          onSubmit={
            openOption === 'builtin'
              ? useBuiltin
              : openOption === 'apikey'
                ? () => void submitCloud()
                : () => void submitLocal()
          }
          onClose={() => setOpenOption(null)}
        />
      )}
    </div>
  );
}
