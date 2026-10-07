import { ExternalLink, ShieldCheck } from 'lucide-react';
import { type ReactNode, useId } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import type { EngineState } from '../../services/api/memoryApi';
import { TERMS_OF_USE_URL } from '../../utils/links';
import { openUrl } from '../../utils/openUrl';
import { Alert, AlertDescription, Button, Label, ModalShell, TextField } from '../ui';
import MemoryProviderLogo, { type MemoryProviderOption } from './MemoryProviderLogo';

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

export interface MemoryProviderModalProps {
  option: MemoryProviderOption;
  title: string;
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
  onClose: () => void;
}

/**
 * Settings for one memory provider: what it is, what it needs (nothing for
 * TinyHumans, a key for Cloud, an endpoint and key for Local), and for
 * TinyHumans the free-ingestion terms.
 */
export default function MemoryProviderModal({
  option,
  title,
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
  onClose,
}: MemoryProviderModalProps) {
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
      <p className="text-sm text-content-secondary">{t('memoryPage.engine.builtin.description')}</p>
      <p className="text-xs text-content-muted">{t('memoryPage.engine.builtin.enrichmentNote')}</p>
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
      <section
        className="rounded-lg border border-line bg-surface-muted p-3.5"
        data-testid="memory-engine-fair-use">
        <h3 className="flex items-center gap-1.5 text-sm font-semibold text-content">
          <ShieldCheck className="h-4 w-4 text-primary-500" aria-hidden />
          {t('memoryPage.engine.freeIngestion.title')}
        </h3>
        <p className="mt-1 text-xs leading-relaxed text-content-secondary">
          {plan === 'BASIC' || plan === 'PRO'
            ? t('memoryPage.engine.freeIngestion.bodyIncluded').replace(
                '{plan}',
                plan === 'PRO' ? 'Pro' : 'Basic'
              )
            : t('memoryPage.engine.freeIngestion.bodyUpgrade')}
        </p>
        <ul className="mt-2 list-disc space-y-1 pl-4 text-xs leading-relaxed text-content-muted">
          <li>{t('memoryPage.engine.fairUse.own')}</li>
          <li>{t('memoryPage.engine.fairUse.noAbuse')}</li>
          <li>{t('memoryPage.engine.fairUse.limits')}</li>
        </ul>
        <p className="mt-2 text-xs">
          <ExternalTextLink href={TERMS_OF_USE_URL} testId="memory-engine-terms">
            {t('memoryPage.engine.fairUse.terms')}
          </ExternalTextLink>
        </p>
      </section>
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
      <p className="text-sm text-content-secondary">
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
      <ol className="list-decimal space-y-1 pl-4 text-sm text-content-secondary">
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
  const showSubmit = option !== 'builtin' || !active;
  const submitLabel =
    saving === option
      ? t('memoryPage.engine.connecting')
      : option === 'builtin'
        ? t('memoryPage.engine.use')
        : active
          ? t('memoryPage.engine.save')
          : t('memoryPage.engine.connect');

  return (
    <ModalShell
      onClose={onClose}
      title={title}
      titleId={`${baseId}-title`}
      subtitle={description}
      icon={<MemoryProviderLogo option={option} className="h-5 w-5" />}
      maxWidthClassName="max-w-lg"
      contentClassName="flex flex-col gap-4 px-5 py-4"
      testId="memory-engine-modal"
      footer={
        <div className="flex w-full items-center justify-end gap-2">
          <Button
            size="sm"
            variant="secondary"
            analyticsId="memory-engine-modal-close"
            data-testid="memory-engine-modal-close"
            onClick={onClose}>
            {t('common.close')}
          </Button>
          {showSubmit && (
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
          )}
        </div>
      }>
      {option === 'builtin' ? builtinBody : option === 'apikey' ? cloudBody : localBody}
      {error && (
        <Alert variant="destructive" data-testid={`memory-engine-${option}-error`}>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
    </ModalShell>
  );
}
