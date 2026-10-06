/**
 * Memory → Engine: memory runs on CortexDB, reached one of three ways, shown as
 * a single-open accordion (#7025):
 *
 * - Built-in: the `tinyhumans` engine, the TinyHumans backend's `/memory/*`
 *   API (CortexDB hosted per account), authenticated by sign-in. The backend
 *   origin it uses is shown read-only from `memory_engine_get`. Signed out (or
 *   on a local session), it says so and cannot be selected.
 * - Your API key: the `cortexdb` engine on CortexDB's managed API. The endpoint
 *   is fixed; only the key is entered.
 * - Self-host: the `cortexdb` engine on a server on this computer. The core only
 *   allows cleartext http to loopback, so the endpoint is checked here first.
 *
 * Which item is active is derived from `memory_engine_get`: `tinyhumans` is
 * Built-in, and `cortexdb` is Self-host when its endpoint is loopback, else
 * API key. The status banner still explains an off, degraded or down engine.
 *
 * debug logging: DEBUG=openhuman:memory:engine
 */
import debug from 'debug';
import { ExternalLink } from 'lucide-react';
import { useCallback, useId, useState } from 'react';

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
import { openUrl } from '../../utils/openUrl';
import {
  AccordionContent,
  AccordionItem,
  AccordionRoot,
  AccordionTrigger,
  Alert,
  AlertDescription,
  AlertTitle,
  Badge,
  Button,
  Label,
  TextField,
} from '../ui';
import { CenteredLoadingState } from '../ui/LoadingState';

const log = debug('openhuman:memory:engine');

/** CortexDB's self-hosting guide, linked from the Self-host item. */
export const CORTEXDB_SELF_HOST_DOCS_URL = 'https://cortexdb.ai/docs/self-hosting/quickstart';

/** CortexDB's managed API, the fixed endpoint of the API-key item. */
const CORTEXDB_CLOUD_ENDPOINT = 'https://api-v1.cortexdb.ai';

/** CortexDB's default port on this computer, shown as the example endpoint. */
const SELF_HOST_EXAMPLE_ENDPOINT = 'http://localhost:3141';

type EngineOption = 'builtin' | 'apikey' | 'selfhost';

/** True for an http(s) URL whose host is this computer (localhost, 127.x, ::1). */
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

/** The accordion item the configured engine corresponds to. */
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
  const baseId = useId();
  const { snapshot } = useCoreState();
  const signedIn = snapshot.auth.isAuthenticated && !isLocalSessionToken(snapshot.sessionToken);

  const active = optionOf(state);
  const on = isMemoryOn(state);

  const [open, setOpen] = useState<string | undefined>(undefined);
  const [saving, setSaving] = useState<EngineOption | null>(null);
  const [errors, setErrors] = useState<Partial<Record<EngineOption, string>>>({});
  const [cloudKey, setCloudKey] = useState('');
  // Untouched (null) shows the configured local endpoint, which may arrive
  // after the first render when a host loads the state itself.
  const [typedEndpoint, setTypedEndpoint] = useState<string | null>(null);
  const [localKey, setLocalKey] = useState('');

  const select = useCallback(
    async (option: EngineOption, req: EngineSetRequest): Promise<boolean> => {
      setSaving(option);
      setErrors(prev => ({ ...prev, [option]: undefined }));
      try {
        const next = await memoryEngineSet(req);
        log('engine set (%s): %s status=%s', option, next.engine ?? 'none', next.status);
        onStateChange(next);
        return true;
      } catch (err) {
        log('engine set (%s) failed: %o', option, err);
        setErrors(prev => ({ ...prev, [option]: memoryErrorMessage(err, t) }));
        return false;
      } finally {
        setSaving(null);
      }
    },
    [onStateChange, t]
  );

  if (!state) {
    return <CenteredLoadingState label={t('memoryPage.loading')} />;
  }

  const statusBanner = (() => {
    if (!on) {
      return (
        <Alert variant="info" data-testid="memory-engine-status-off">
          <AlertTitle>{t('memoryPage.off.title')}</AlertTitle>
          <AlertDescription>
            {state.reason || t('memoryPage.engine.offExplanation')}
          </AlertDescription>
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

  const activeBadge = (option: EngineOption) => {
    if (option !== active) return null;
    const [variant, label] = !on
      ? (['warning', t('memoryPage.engine.statusOff')] as const)
      : state.status === 'down'
        ? (['danger', t('memoryPage.engine.badgeDown')] as const)
        : state.status === 'degraded'
          ? (['warning', t('memoryPage.engine.badgeDegraded')] as const)
          : (['success', t('memoryPage.engine.active')] as const);
    return (
      <Badge variant={variant} data-testid={`memory-engine-${option}-active`}>
        {label}
      </Badge>
    );
  };

  const itemError = (option: EngineOption) =>
    errors[option] ? (
      <Alert variant="destructive" data-testid={`memory-engine-${option}-error`}>
        <AlertDescription>{errors[option]}</AlertDescription>
      </Alert>
    ) : null;

  const actionLabel = (option: EngineOption) =>
    saving === option
      ? t('memoryPage.engine.connecting')
      : option === active
        ? t('memoryPage.engine.save')
        : t('memoryPage.engine.connect');

  const keyField = (
    option: 'apikey' | 'selfhost',
    value: string,
    onChange: (value: string) => void
  ) => {
    const saved = option === active && state.has_key;
    return (
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${baseId}-${option}-key`} className="text-xs text-content-secondary">
          {t('memoryPage.engine.apiKey')}
        </Label>
        <TextField
          id={`${baseId}-${option}-key`}
          data-testid={`memory-engine-${option}-key`}
          type="password"
          mono
          autoComplete="off"
          spellCheck={false}
          data-lpignore="true"
          data-1p-ignore="true"
          value={value}
          disabled={saving !== null}
          placeholder={saved ? t('memoryPage.engine.keySavedPlaceholder') : ''}
          onChange={e => onChange(e.target.value)}
        />
        {saved && (
          <p className="text-[11px] leading-4 text-content-muted">
            {t('memoryPage.engine.keySavedHint')}
          </p>
        )}
      </div>
    );
  };

  // Built-in.
  const builtinBlocked = !signedIn;
  const builtinIdle = active === 'builtin' && on;

  // API key: the endpoint is CortexDB's managed API. Sending a blank endpoint
  // clears any custom (self-host) endpoint, so the engine falls back to it.
  const cloudKeyRequired = !(active === 'apikey' && state.has_key);
  const canSubmitCloud = saving === null && (!cloudKeyRequired || cloudKey.trim().length > 0);
  const submitCloud = async () => {
    if (!canSubmitCloud) return;
    const req: EngineSetRequest = { engine: 'cortexdb', endpoint: '' };
    if (cloudKey.trim()) req.api_key = cloudKey.trim();
    if (await select('apikey', req)) setCloudKey('');
  };

  // Self-host: loopback only.
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

  const trigger = (option: EngineOption, title: string, detail: string) => (
    <AccordionTrigger data-testid={`memory-engine-${option}-trigger`}>
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex items-center gap-2">
          {title}
          {activeBadge(option)}
        </span>
        <span className="truncate text-xs font-normal text-content-muted">{detail}</span>
      </span>
    </AccordionTrigger>
  );

  return (
    <div
      className={embedded ? 'space-y-4' : 'space-y-4 animate-fade-up'}
      data-testid="memory-engine-tab">
      {statusBanner}

      <div className="space-y-2">
        <div>
          <h3 className="text-sm font-semibold text-content">{t('memoryPage.engine.listTitle')}</h3>
          <p className="text-xs text-content-muted">{t('memoryPage.engine.listDescription')}</p>
        </div>

        <AccordionRoot
          type="single"
          collapsible
          variant="card"
          value={open ?? active ?? 'builtin'}
          onValueChange={setOpen}
          data-testid="memory-engines">
          <AccordionItem value="builtin" variant="card" data-testid="memory-engine-builtin">
            {trigger(
              'builtin',
              t('memoryPage.engine.builtin.title'),
              builtinBlocked
                ? t('memoryPage.engine.builtin.signInRequired')
                : t('memoryPage.engine.builtin.detail')
            )}
            <AccordionContent className="space-y-3">
              <p>{t('memoryPage.engine.builtin.description')}</p>
              <p className="text-xs text-content-muted">
                {t('memoryPage.engine.builtin.enrichmentNote')}
              </p>
              {/* The backend origin the core resolved, read-only: never a
                  hard-coded URL, and only known while Built-in is configured. */}
              {active === 'builtin' && state.endpoint && (
                <p className="text-xs text-content-muted">
                  {t('memoryPage.engine.endpoint')}:{' '}
                  <span className="font-mono" data-testid="memory-engine-builtin-endpoint">
                    {state.endpoint}
                  </span>
                </p>
              )}
              {builtinBlocked && (
                <p
                  className="text-xs text-content-muted"
                  data-testid="memory-engine-builtin-sign-in">
                  {t('memoryPage.engine.builtin.signInHint')}
                </p>
              )}
              {itemError('builtin')}
              {!builtinIdle && (
                <Button
                  type="button"
                  size="sm"
                  variant="primary"
                  disabled={saving !== null || builtinBlocked}
                  data-testid="memory-engine-builtin-use"
                  onClick={() => void select('builtin', { engine: 'tinyhumans' })}>
                  {saving === 'builtin'
                    ? t('memoryPage.engine.connecting')
                    : t('memoryPage.engine.use')}
                </Button>
              )}
            </AccordionContent>
          </AccordionItem>

          <AccordionItem value="apikey" variant="card" data-testid="memory-engine-apikey">
            {trigger('apikey', t('memoryPage.engine.apiKeyOption.title'), CORTEXDB_CLOUD_ENDPOINT)}
            <AccordionContent>
              <form
                className="flex flex-col gap-3"
                onSubmit={event => {
                  event.preventDefault();
                  void submitCloud();
                }}>
                <p>{t('memoryPage.engine.apiKeyOption.description')}</p>
                {keyField('apikey', cloudKey, setCloudKey)}
                {itemError('apikey')}
                <div>
                  <Button
                    type="submit"
                    size="sm"
                    variant="primary"
                    disabled={!canSubmitCloud}
                    data-testid="memory-engine-apikey-submit">
                    {actionLabel('apikey')}
                  </Button>
                </div>
              </form>
            </AccordionContent>
          </AccordionItem>

          <AccordionItem value="selfhost" variant="card" data-testid="memory-engine-selfhost">
            {trigger(
              'selfhost',
              t('memoryPage.engine.selfHost.title'),
              active === 'selfhost' && state.endpoint
                ? state.endpoint
                : t('memoryPage.engine.selfHost.detail')
            )}
            <AccordionContent>
              <form
                className="flex flex-col gap-3"
                onSubmit={event => {
                  event.preventDefault();
                  void submitLocal();
                }}>
                <ol className="list-decimal space-y-1 pl-4">
                  <li>
                    {t('memoryPage.engine.selfHost.step1')}{' '}
                    <a
                      href={CORTEXDB_SELF_HOST_DOCS_URL}
                      target="_blank"
                      rel="noopener noreferrer"
                      data-testid="memory-engine-selfhost-docs"
                      onClick={event => {
                        event.preventDefault();
                        void openUrl(CORTEXDB_SELF_HOST_DOCS_URL).catch(() => undefined);
                      }}
                      className="inline-flex items-center gap-1 font-medium text-primary-600 hover:underline dark:text-primary-300">
                      {t('memoryPage.engine.selfHost.docsLink')}
                      <ExternalLink className="h-3 w-3" aria-hidden />
                    </a>
                  </li>
                  <li>{t('memoryPage.engine.selfHost.step2')}</li>
                  <li>{t('memoryPage.engine.selfHost.step3')}</li>
                </ol>
                <div className="flex flex-col gap-1.5">
                  <Label
                    htmlFor={`${baseId}-selfhost-endpoint`}
                    className="text-xs text-content-secondary">
                    {t('memoryPage.engine.endpoint')}
                  </Label>
                  <TextField
                    id={`${baseId}-selfhost-endpoint`}
                    data-testid="memory-engine-selfhost-endpoint"
                    type="url"
                    mono
                    spellCheck={false}
                    value={localEndpoint}
                    disabled={saving !== null}
                    placeholder={SELF_HOST_EXAMPLE_ENDPOINT}
                    onChange={e => setTypedEndpoint(e.target.value)}
                  />
                  {endpointTyped && !endpointLocal && (
                    <p
                      className="text-[11px] leading-4 text-destructive"
                      data-testid="memory-engine-selfhost-endpoint-error">
                      {t('memoryPage.engine.selfHost.notLocal')}
                    </p>
                  )}
                </div>
                {keyField('selfhost', localKey, setLocalKey)}
                {itemError('selfhost')}
                <div>
                  <Button
                    type="submit"
                    size="sm"
                    variant="primary"
                    disabled={!canSubmitLocal}
                    data-testid="memory-engine-selfhost-submit">
                    {actionLabel('selfhost')}
                  </Button>
                </div>
              </form>
            </AccordionContent>
          </AccordionItem>
        </AccordionRoot>
      </div>
    </div>
  );
}
