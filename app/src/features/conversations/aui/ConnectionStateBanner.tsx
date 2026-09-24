/**
 * The thread's connection banner: assistant-ui's `connection-state` element
 * over the renderer's Socket.IO link to the core (`socket` slice, written by
 * `socketService`'s connect / disconnect / connect_error handlers).
 *
 * Phase mapping (`selectSocketStatus` → element `phase`):
 * - `connected`                        → `online` (renders nothing)
 * - `disconnected` after being live    → `dropped`, with Reconnect
 * - `connecting` after being live      → `reconnecting`
 * - `connected` after a drop           → `resumed` for `RESUMED_VISIBLE_MS`,
 *                                        then `online`
 *
 * Before this banner has seen the socket connect at all (app boot, a thread
 * opened mid-outage) it stays quiet: that is a cold start, not a dropped
 * stream, and the app-level connectivity chip already reports it.
 *
 * "Resumed" is derived from the socket's reconnect, not from the thread's
 * replay finishing. On `connected`, `socketService` rejoins the thread rooms
 * (`thread:subscribe`) and `ChatRuntimeProvider` re-reads interrupted threads,
 * but neither publishes a "replay done" signal to observe.
 *
 * Reconnect reuses `socketService.connect(sessionToken)`, the same entry point
 * `SocketProvider` and the Activity page use; it flips the status to
 * `connecting`, which moves the banner to `reconnecting`.
 */
import debugFactory from 'debug';
import { useContext, useEffect, useRef, useState } from 'react';
import { ReactReduxContext } from 'react-redux';

import {
  type ConnectionPhase,
  ConnectionState,
} from '../../../components/assistant-ui/elements/connection-state';
import { getCoreStateSnapshot } from '../../../lib/coreState/store';
import { useT } from '../../../lib/i18n/I18nContext';
import { socketService } from '../../../services/socketService';
import { useAppSelector } from '../../../store/hooks';
import { selectSocketStatus } from '../../../store/socketSelectors';

const log = debugFactory('openhuman:aui:connection-state');

/** How long "Picked the stream back up." stays before the banner clears. */
export const RESUMED_VISIBLE_MS = 3000;

type SocketStatus = ReturnType<typeof selectSocketStatus>;

function useConnectionPhase(status: SocketStatus): ConnectionPhase {
  const [phase, setPhase] = useState<ConnectionPhase>('online');
  const everConnected = useRef(false);
  const droppedSinceConnect = useRef(false);

  useEffect(() => {
    if (status === 'connected') {
      const resumed = droppedSinceConnect.current;
      everConnected.current = true;
      droppedSinceConnect.current = false;
      if (resumed) log('socket reconnected → resumed');
      setPhase(resumed ? 'resumed' : 'online');
      return;
    }
    if (!everConnected.current) return;
    droppedSinceConnect.current = true;
    const next: ConnectionPhase = status === 'connecting' ? 'reconnecting' : 'dropped';
    log('socket %s → %s', status, next);
    setPhase(next);
  }, [status]);

  useEffect(() => {
    if (phase !== 'resumed') return;
    const timer = setTimeout(() => setPhase('online'), RESUMED_VISIBLE_MS);
    return () => clearTimeout(timer);
  }, [phase]);

  return phase;
}

function reconnect() {
  const token = getCoreStateSnapshot().snapshot?.sessionToken;
  if (!token) {
    log('reconnect skipped: no session token');
    return;
  }
  log('reconnect requested');
  socketService.connect(token);
}

/** The element with translated captions, for a given phase (also the dev gallery's fixture). */
export function ConnectionStateNotice({
  phase,
  onRetry,
}: {
  phase: ConnectionPhase;
  onRetry?: () => void;
}) {
  const { t } = useT();
  return (
    <ConnectionState
      data-testid="connection-state-banner"
      phase={phase}
      onRetry={onRetry}
      droppedLabel={t('chat.connectionState.dropped')}
      retryLabel={t('chat.connectionState.reconnect')}
      reconnectingLabel={t('chat.connectionState.reconnecting')}
      resumedLabel={t('chat.connectionState.resumed')}
    />
  );
}

function ConnectedConnectionStateBanner({ status }: { status: SocketStatus }) {
  const phase = useConnectionPhase(status);
  return <ConnectionStateNotice phase={phase} onRetry={reconnect} />;
}

/** `null` under a host store that carries no `socket` slice. */
const selectSocketStatusIfTracked = (state: RootState): SocketStatus | null =>
  state.socket ? selectSocketStatus(state) : null;

function StoreConnectionStateBanner() {
  const status = useAppSelector(selectSocketStatusIfTracked);
  if (status === null) return null;
  return <ConnectedConnectionStateBanner status={status} />;
}

/**
 * Renders nothing when there is no socket state above it: the thread is also
 * mounted standalone or under a partial host store (dev demo, component
 * tests), with no socket to report on.
 */
export function ConnectionStateBanner() {
  const redux = useContext(ReactReduxContext);
  if (!redux) return null;
  return <StoreConnectionStateBanner />;
}
