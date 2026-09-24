'use client';

/**
 * OpenHuman glue over the vendored `elements/permission-grant.tsx`, replacing
 * the deleted `IntegrationConnectCard`'s card body while preserving every bit
 * of its OAuth-launch behavior (per the assistant-ui-elements plan, WS-B row:
 * "replacing IntegrationConnectCard's card body but preserving its
 * OAuth-launch behavior ... port it into the adapter, not into the vendored
 * element").
 *
 * Rendered by `ChatToolParts.tsx`'s `ComposioConnectCall` in place of the
 * parked `composio_connect` tool call — the same `approval_request` socket
 * path as every other gated tool, but "Approve" is the wrong affordance
 * (approving without connecting resumes the agent against a toolkit with no
 * credentials).
 *
 * Provider-specific required fields (WhatsApp `waba_id`, Jira `subdomain`,
 * Dynamics 365 `org_name`) are OpenHuman-specific and have no equivalent in
 * the vendored element's `reach` list, so they render as adapter-owned inputs
 * ABOVE the `PermissionGrant` element rather than inside it — the vendored
 * markup itself is untouched apart from the label/slot props documented on
 * `permission-grant.tsx`.
 *
 * The vendored element's three-way decision (`GrantScope`: session / always /
 * denied) doesn't map onto this binary OAuth flow (there is no "grant for
 * this session only" — a connection is either live or it isn't), so both
 * "This session" and "Always" route to the same `connect()` call and "Deny"
 * is the only way to cancel. This is a deliberate, documented simplification
 * rather than a restyle of the element.
 */
import debug from 'debug';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { PermissionGrant } from '../../../components/assistant-ui/elements/permission-grant';
import {
  getRequiredFieldsForToolkit,
  validateRequiredFieldValues,
} from '../../../components/composio/toolkitRequiredFields';
import { TextField } from '../../../components/ui';
import { authorize, listConnections } from '../../../lib/composio/composioApi';
import { canonicalizeComposioToolkitSlug } from '../../../lib/composio/toolkitSlug';
import { deriveComposioState } from '../../../lib/composio/types';
import { useT } from '../../../lib/i18n/I18nContext';
import { callCoreRpc } from '../../../services/coreRpcClient';
import {
  clearPendingApprovalForThread,
  type PendingApproval,
} from '../../../store/chatRuntimeSlice';
import { useAppDispatch } from '../../../store/hooks';
import { openUrl } from '../../../utils/openUrl';

const log = debug('openhuman:aui:permission-grant-adapter');

const POLL_INTERVAL_MS = 4_000;
const POLL_TIMEOUT_MS = 5 * 60 * 1_000;
const MISSING_REQUIRED_FIELDS_SLUG = 'ConnectedAccount_MissingRequiredFields';

type Phase = 'idle' | 'connecting' | 'error';

interface Props {
  threadId: string;
  approval: PendingApproval;
}

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function PermissionGrantAdapter({ threadId, approval }: Props) {
  const { t } = useT();
  const dispatch = useAppDispatch();
  const toolkit = canonicalizeComposioToolkitSlug(approval.toolkit ?? '');

  const [phase, setPhase] = useState<Phase>('idle');
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [retryable, setRetryable] = useState(true);

  const requiredFields = useMemo(() => getRequiredFieldsForToolkit(toolkit), [toolkit]);
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});

  const pollTimerRef = useRef<number | null>(null);
  const pollDeadlineRef = useRef<number>(0);
  const isPollingRef = useRef<boolean>(false);
  const inFlightRef = useRef<boolean>(false);
  const cancelledRef = useRef<boolean>(false);

  const stopPolling = useCallback(() => {
    isPollingRef.current = false;
    if (pollTimerRef.current != null) {
      window.clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  }, []);

  useEffect(
    () => () => {
      cancelledRef.current = true;
      stopPolling();
    },
    [stopPolling]
  );

  const resolveGate = useCallback(
    async (decision: 'approve_once' | 'deny') => {
      try {
        await callCoreRpc({
          method: 'openhuman.approval_decide',
          params: { request_id: approval.requestId, decision },
        });
      } catch (e) {
        log('approval_decide(%s) failed: %o', decision, e);
        setPhase('error');
        setErrorMsg(t('chat.approval.error'));
        return;
      }
      dispatch(clearPendingApprovalForThread({ threadId }));
    },
    [approval.requestId, dispatch, threadId, t]
  );

  const startPolling = useCallback(() => {
    stopPolling();
    isPollingRef.current = true;
    pollDeadlineRef.current = Date.now() + POLL_TIMEOUT_MS;

    const scheduleNext = () => {
      if (!isPollingRef.current) return;
      pollTimerRef.current = window.setTimeout(() => void tick(), POLL_INTERVAL_MS);
    };

    const tick = async () => {
      if (inFlightRef.current || !isPollingRef.current) return;
      if (Date.now() > pollDeadlineRef.current) {
        stopPolling();
        setPhase('error');
        setErrorMsg(t('composio.connect.oauthTimeout'));
        await resolveGate('deny');
        return;
      }
      inFlightRef.current = true;
      try {
        const resp = await listConnections();
        const matches = resp.connections.filter(
          c => c.toolkit.toLowerCase() === toolkit.toLowerCase()
        );
        if (matches.some(c => deriveComposioState(c) === 'connected')) {
          stopPolling();
          await resolveGate('approve_once');
          return;
        }
        const pending = matches.some(c => deriveComposioState(c) === 'pending');
        const errored = matches.find(c => deriveComposioState(c) === 'error');
        if (errored && !pending) {
          stopPolling();
          setPhase('error');
          setErrorMsg(
            t('composio.connect.connectionFailed').replace('{status}', String(errored.status))
          );
          return;
        }
      } catch (err) {
        log('connection poll failed: %o', err);
      } finally {
        inFlightRef.current = false;
      }
      scheduleNext();
    };

    void tick();
  }, [resolveGate, stopPolling, t, toolkit]);

  const connect = useCallback(async () => {
    if (phase === 'connecting' || !toolkit) return;
    cancelledRef.current = false;

    let extraParams: Record<string, string> | undefined;
    if (requiredFields.length > 0) {
      const errors = validateRequiredFieldValues(requiredFields, fieldValues);
      if (Object.keys(errors).length > 0) {
        setFieldErrors(errors);
        return;
      }
      setFieldErrors({});
      extraParams = {};
      for (const f of requiredFields) {
        extraParams[f.key] = (fieldValues[f.key] ?? '').trim();
      }
    }

    setPhase('connecting');
    setErrorMsg(null);
    setRetryable(true);
    try {
      const resp = await authorize(toolkit, extraParams);
      if (cancelledRef.current) return;
      try {
        await openUrl(resp.connectUrl);
      } catch (openErr) {
        log('openUrl failed: %o', openErr);
      }
      startPolling();
    } catch (e) {
      log('authorize failed: %o', e);
      setPhase('error');
      if (errorText(e).includes(MISSING_REQUIRED_FIELDS_SLUG) && requiredFields.length === 0) {
        setErrorMsg(t('composio.connect.additionalConfigRequired'));
      } else {
        const base = t('composio.connect.connectionFailed')
          .replace(/\s*\([^)]*\{status\}[^)]*\)/, '')
          .trim();
        const reason = errorText(e).replace(/\s+/g, ' ').trim().slice(0, 240);
        setErrorMsg(reason ? `${base} ${reason}` : base);
        if (/no auth config|not a valid toolkit|unknown toolkit|not found|\b400\b/i.test(reason)) {
          setRetryable(false);
        }
      }
    }
  }, [phase, requiredFields, fieldValues, startPolling, t, toolkit]);

  const cancel = useCallback(async () => {
    cancelledRef.current = true;
    stopPolling();
    await resolveGate('deny');
  }, [resolveGate, stopPolling]);

  const connecting = phase === 'connecting';
  const showFields = requiredFields.length > 0 && !connecting;
  const showConnect = !(phase === 'error' && !retryable);

  return (
    <div
      role="group"
      aria-label={approval.message || t('composio.connect.connect')}
      data-testid="assistant-ui-integration-connect">
      {showFields && (
        <div className="mb-2.5 flex flex-col gap-2.5">
          {requiredFields.map(f => (
            <label key={f.key} className="block text-xs text-content-secondary">
              <span className="font-medium">{t(f.labelKey)}</span>
              <span className="mt-1 flex items-center gap-1.5">
                <TextField
                  type="text"
                  value={fieldValues[f.key] ?? ''}
                  placeholder={f.placeholderKey ? t(f.placeholderKey) : undefined}
                  onChange={e => setFieldValues(prev => ({ ...prev, [f.key]: e.target.value }))}
                  className="min-w-0 flex-1"
                />
                {f.suffix && <span className="shrink-0 text-content-faint">{f.suffix}</span>}
              </span>
              {f.hintKey && <span className="mt-1 block text-content-muted">{t(f.hintKey)}</span>}
              {fieldErrors[f.key] && (
                <span className="mt-1 block text-coral-600 dark:text-coral-400">
                  {t(fieldErrors[f.key])}
                </span>
              )}
            </label>
          ))}
        </div>
      )}

      <PermissionGrant
        capability={approval.message || t('chat.approval.fallback')}
        requester={approval.toolName}
        requesterLabel={t('chat.approval.tool')}
        reach={connecting ? [t('composio.connect.waitingHint')] : []}
        scope={connecting ? 'busy' : 'pending'}
        onGrant={
          showConnect
            ? scope => {
                // Only "Always" actually connects — this OAuth handoff is
                // binary (live or not), so both "Deny" and "This session"
                // cancel the same way. Distinct labels keep exactly one
                // button reading "Connect", which is what a user (and this
                // component's own test suite) looks for.
                if (scope === 'always') void connect();
                else void cancel();
              }
            : undefined
        }
        denyLabel={t('chat.approval.deny')}
        sessionLabel={t('chat.approval.deny')}
        alwaysLabel={
          phase === 'error' ? t('composio.connect.retryConnection') : t('composio.connect.connect')
        }
        pendingLabel={t('chat.approval.deciding')}
        denyProps={{ 'data-analytics-id': 'chat-integration-connect-cancel' }}
        sessionProps={{ 'data-analytics-id': 'chat-integration-connect-cancel' }}
        alwaysProps={{ 'data-analytics-id': 'chat-integration-connect', disabled: !toolkit }}
      />

      {errorMsg && <p className="mt-2 text-xs text-coral-600 dark:text-coral-400">⚠ {errorMsg}</p>}
    </div>
  );
}
