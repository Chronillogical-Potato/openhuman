import { Check, ExternalLink, Gift } from 'lucide-react';
import { type ReactNode, useId } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import { type EngineState, isMemoryOn } from '../../services/api/memoryApi';
import { TERMS_OF_USE_URL } from '../../utils/links';
import { openUrl } from '../../utils/openUrl';
import { Alert, AlertDescription, Button, Label, TextField } from '../ui';
import type { MemoryProviderOption } from './MemoryProviderLogo';

/** CortexDB's self-hosting guide, linked from the Local provider. */
export const CORTEXDB_SELF_HOST_DOCS_URL = 'https://cortexdb.ai/docs/self-hosting/quickstart';

/** CortexDB's default port on this computer, shown as the example endpoint. */
const SELF_HOST_EXAMPLE_ENDPOINT = 'http://localhost:3141';

const ExternalTextLink = ({
  href,
  testId,
  children,
}: {
  href: string;
  testId: string;
  children: ReactNode;
}) => (
  <a
    href={href}
    target="_blank"
    rel="noopener noreferrer"
    data-testid={testId}
    onClick={event => {
      event.preventDefault();
      void openUrl(href).catch(() => undefined);
    }}
    className="inline-flex items-center gap-1 font-medium text-primary-600 hover:underline dark:text-primary-300">
    {children}
    <ExternalLink className="h-3 w-3" aria-hidden />
  </a>
);

export interface MemoryConnectionPanelProps {
  option: MemoryProviderOption;
  description: string;
  state: EngineState;
  /** True when this provider is the configured one. */
  active: boolean;
  signedIn: boolean;
  /** The signed-in user's plan, when known (FREE / BASIC / PRO). */
  plan: string | null;
  /** The option currently being saved, if any. */
  saving: MemoryProviderOption | null;
  error?: string;
  cloudKey: string;
  onCloudKey: (value: string) => void;
  localEndpoint: string;
  onLocalEndpoint: (value: string) => void;
  endpointInvalid: boolean;
  localKey: string;
  onLocalKey: (value: string) => void;
  /** Whether the footer's primary button can be pressed. */
  canSubmit: boolean;
  onSubmit: () => void;
}

/**
 * One way to connect CortexDB, shown under its chip on Memory → Provider: what
 * it is, what it needs (nothing via TinyHumans, a key for your own account, an
 * endpoint and key for Local), its action, and via TinyHumans the free
 * ingestion note with the fair-use terms.
 */
export default function MemoryConnectionPanel({
  option,
  description,
  state,
  active,
  signedIn,
  plan,
  saving,
  error,
  cloudKey,
  onCloudKey,
  localEndpoint,
  onLocalEndpoint,
  endpointInvalid,
  localKey,
  onLocalKey,
  canSubmit,
  onSubmit,
}: MemoryConnectionPanelProps) {
  const { t } = useT();
  const baseId = useId();
  const formId = `${baseId}-form`;
  const busy = saving !== null;

  const keyField = (value: string, onChange: (value: string) => void) => {
    const saved = active && state.has_key;
    return (
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${baseId}-key`} className="text-xs text-content-secondary">
          {t('memoryPage.engine.apiKey')}
        </Label>
        <TextField
          id={`${baseId}-key`}
          data-testid={`memory-engine-${option}-key`}
          type="password"
          mono
          autoComplete="off"
          spellCheck={false}
          data-lpignore="true"
          data-1p-ignore="true"
          value={value}
          disabled={busy}
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

  const builtinBody = (
    <>
      <p className="text-xs leading-relaxed text-content-secondary">{description}</p>
      <p
        className="flex items-center gap-1.5 text-xs text-content-secondary"
        data-testid="memory-engine-builtin-note">
        <Gift className="h-3.5 w-3.5 shrink-0 text-primary-500" aria-hidden />
        {plan === 'BASIC' || plan === 'PRO'
          ? t('memoryPage.engine.freeIngestion.notePlan').replace(
              '{plan}',
              plan === 'PRO' ? 'Pro' : 'Basic'
            )
          : t('memoryPage.engine.freeIngestion.noteUpgrade')}
      </p>
      <details className="group text-xs" data-testid="memory-engine-fair-use">
        <summary className="cursor-pointer font-medium text-content-secondary select-none hover:text-content">
          {t('memoryPage.engine.fairUse.summary')}
        </summary>
        <ul className="mt-1.5 list-disc space-y-1 pl-4 leading-relaxed text-content-muted">
          <li>{t('memoryPage.engine.fairUse.own')}</li>
          <li>{t('memoryPage.engine.fairUse.noAbuse')}</li>
          <li>{t('memoryPage.engine.fairUse.limits')}</li>
        </ul>
        <p className="mt-1.5">
          <ExternalTextLink href={TERMS_OF_USE_URL} testId="memory-engine-terms">
            {t('memoryPage.engine.fairUse.terms')}
          </ExternalTextLink>
        </p>
      </details>
      {/* The backend origin the core resolved, read-only: never a hard-coded
          URL, and only known while TinyHumans is configured. */}
      {active && state.endpoint && (
        <p className="text-xs text-content-muted">
          {t('memoryPage.engine.endpoint')}:{' '}
          <span className="font-mono" data-testid="memory-engine-builtin-endpoint">
            {state.endpoint}
          </span>
        </p>
      )}
      {!signedIn && (
        <p className="text-xs text-content-muted" data-testid="memory-engine-builtin-sign-in">
          {t('memoryPage.engine.builtin.signInHint')}
        </p>
      )}
    </>
  );

  const cloudBody = (
    <form
      id={formId}
      className="flex flex-col gap-3"
      onSubmit={event => {
        event.preventDefault();
        onSubmit();
      }}>
      <p className="text-xs leading-relaxed text-content-secondary">
        {t('memoryPage.engine.apiKeyOption.description')}
      </p>
      {keyField(cloudKey, onCloudKey)}
    </form>
  );

  const localBody = (
    <form
      id={formId}
      className="flex flex-col gap-3"
      onSubmit={event => {
        event.preventDefault();
        onSubmit();
      }}>
      <ol className="list-decimal space-y-1 pl-4 text-xs leading-relaxed text-content-secondary">
        <li>
          {t('memoryPage.engine.selfHost.step1')}{' '}
          <ExternalTextLink href={CORTEXDB_SELF_HOST_DOCS_URL} testId="memory-engine-selfhost-docs">
            {t('memoryPage.engine.selfHost.docsLink')}
          </ExternalTextLink>
        </li>
        <li>{t('memoryPage.engine.selfHost.step2')}</li>
        <li>{t('memoryPage.engine.selfHost.step3')}</li>
      </ol>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor={`${baseId}-endpoint`} className="text-xs text-content-secondary">
          {t('memoryPage.engine.endpoint')}
        </Label>
        <TextField
          id={`${baseId}-endpoint`}
          data-testid="memory-engine-selfhost-endpoint"
          type="url"
          mono
          spellCheck={false}
          value={localEndpoint}
          disabled={busy}
          placeholder={SELF_HOST_EXAMPLE_ENDPOINT}
          onChange={e => onLocalEndpoint(e.target.value)}
        />
        {endpointInvalid && (
          <p
            className="text-[11px] leading-4 text-destructive"
            data-testid="memory-engine-selfhost-endpoint-error">
            {t('memoryPage.engine.selfHost.notLocal')}
          </p>
        )}
      </div>
      {keyField(localKey, onLocalKey)}
    </form>
  );

  // TinyHumans has nothing to save once it is in use; the others always can
  // (a new key, a moved endpoint).
  const showSubmit = option !== 'builtin' || !(active && isMemoryOn(state));
  const submitLabel =
    saving === option
      ? t('memoryPage.engine.connecting')
      : option === 'builtin'
        ? t('memoryPage.engine.use')
        : active
          ? t('memoryPage.engine.save')
          : t('memoryPage.engine.connect');

  return (
    <div
      className="flex flex-col gap-3"
      role="tabpanel"
      data-testid={`memory-engine-panel-${option}`}>
      {option === 'builtin' ? builtinBody : option === 'apikey' ? cloudBody : localBody}
      {error && (
        <Alert variant="destructive" data-testid={`memory-engine-${option}-error`}>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
      <div className="flex items-center justify-end gap-2">
        {showSubmit ? (
          <Button
            size="sm"
            type={option === 'builtin' ? 'button' : 'submit'}
            form={option === 'builtin' ? undefined : formId}
            analyticsId={`memory-engine-${option}-submit`}
            data-testid={`memory-engine-${option}-submit`}
            disabled={!canSubmit}
            onClick={option === 'builtin' ? onSubmit : undefined}>
            {submitLabel}
          </Button>
        ) : (
          <span
            className="inline-flex h-8 items-center gap-1 text-xs font-semibold text-primary-600 dark:text-primary-300"
            data-testid={`memory-engine-${option}-in-use`}>
            <Check className="h-4 w-4" aria-hidden />
            {t('memoryPage.engine.inUse')}
          </span>
        )}
      </div>
    </div>
  );
}
