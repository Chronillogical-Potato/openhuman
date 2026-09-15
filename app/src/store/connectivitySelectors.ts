import type { HostedState } from './connectivitySlice';
import { RootState } from './index';

/**
 * Single app-level "what is broken right now?" derived state. Order matters —
 * the user-blocking outage wins over the soft "we're reconnecting" state.
 *
 * - `internet-offline`  : navigator.onLine = false. Nothing else can talk.
 * - `core-unreachable`  : local sidecar isn't answering. App is dead-in-the-water.
 * - `backend-only`      : the renderer's Socket.IO link to the core is down but
 *                         the core is alive — the app stays usable, we just
 *                         show a soft banner.
 * - `hosted-degraded`   : the core's own link to the hosted backend is down and
 *                         being retried. Chat keeps working (it rides the local
 *                         core bridge); webhooks, Composio triggers, managed-DM
 *                         routing and hosted-brain runs pause until it heals.
 *                         Soft chip only (#6256).
 * - `ok`                : everything healthy.
 */
type BlockingState =
  | 'internet-offline'
  | 'core-unreachable'
  | 'backend-only'
  | 'hosted-degraded'
  | 'ok';

/**
 * A hosted link that is down *and being retried*. `unknown` — the core's
 * reconnect loop is not running (signed out, local session, early boot) — is
 * deliberately excluded: it is the absence of a link, not an outage.
 */
export const isHostedDegraded = (hosted: HostedState | undefined): boolean =>
  hosted === 'connecting' || hosted === 'reconnecting' || hosted === 'error';

export const selectBlockingState = (s: RootState): BlockingState => {
  if (s.connectivity.internet === 'offline') return 'internet-offline';
  if (s.connectivity.core === 'unreachable') return 'core-unreachable';
  if (s.connectivity.backend === 'disconnected' || s.connectivity.backend === 'connecting') {
    return 'backend-only';
  }
  if (isHostedDegraded(s.connectivity.hosted)) return 'hosted-degraded';
  return 'ok';
};
