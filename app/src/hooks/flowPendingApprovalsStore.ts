import debug from 'debug';
import { useCallback, useEffect, useSyncExternalStore } from 'react';

import { fetchPendingApprovals, type PendingApproval } from '../services/api/approvalApi';

const log = debug('flows:pending-approvals-store');
/**
 * Default cadence, and the one every flow consumer uses: a run the user is
 * watching should feel live.
 */
const POLL_INTERVAL_MS = 2000;

export interface FlowPendingApprovalsSnapshot {
  approvals: PendingApproval[];
  error: string | null;
  polling: boolean;
}

function cloneAndFreeze<T>(value: T): T {
  if (value === null || typeof value !== 'object') return value;
  if (Array.isArray(value)) {
    return Object.freeze(value.map(item => cloneAndFreeze(item))) as T;
  }
  return Object.freeze(
    Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, item]) => [
        key,
        cloneAndFreeze(item),
      ])
    )
  ) as T;
}

const freezeApprovals = (approvals: PendingApproval[]): PendingApproval[] =>
  cloneAndFreeze(approvals);

const makeSnapshot = (
  approvals: PendingApproval[],
  error: string | null,
  polling: boolean
): FlowPendingApprovalsSnapshot =>
  Object.freeze({ approvals: freezeApprovals(approvals), error, polling });

const INITIAL_SNAPSHOT = makeSnapshot([], null, false);

let snapshot = INITIAL_SNAPSHOT;
/**
 * One entry per active consumer, holding the cadence it asked for. The poll
 * runs at the FASTEST cadence anyone wants, so a slow background consumer
 * never delays a flow run the user is watching, and a fast flow consumer
 * leaving does not strand the queue at its cadence.
 *
 * An array rather than a count because consumers now differ: the background
 * approvals surface is mounted for the whole session and only needs to notice
 * a 600 s park, while a flow inspector wants 2 s. Polling a table that is
 * almost always empty every 2 s for an entire session, to catch something with
 * a ten-minute lifetime, is cost with no benefit.
 */
let retainedIntervals: number[] = [];
let pollTimer: number | undefined;
let requestGeneration = 0;
let nextRequestId = 0;
let activeRequestId: number | null = null;
let inFlight: Promise<void> | null = null;
let queuedRefresh: Promise<void> | null = null;
const listeners = new Set<() => void>();

function emit(next: FlowPendingApprovalsSnapshot): void {
  snapshot = next;
  for (const listener of listeners) listener();
}

function normalizeError(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === 'string' && error.trim()) return error;
  return 'Unable to load pending approvals';
}

function clearPollTimer(): void {
  if (pollTimer === undefined) return;
  window.clearTimeout(pollTimer);
  pollTimer = undefined;
}

/** The fastest cadence any active consumer asked for. */
function effectiveIntervalMs(): number {
  return retainedIntervals.length === 0 ? POLL_INTERVAL_MS : Math.min(...retainedIntervals);
}

function scheduleNextPoll(generation: number): void {
  if (
    retainedIntervals.length === 0 ||
    generation !== requestGeneration ||
    pollTimer !== undefined
  ) {
    return;
  }
  pollTimer = window.setTimeout(() => {
    pollTimer = undefined;
    void startRefresh(true);
  }, effectiveIntervalMs());
}

export function subscribeFlowPendingApprovals(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function getFlowPendingApprovalsSnapshot(): FlowPendingApprovalsSnapshot {
  return snapshot;
}

function startRefresh(publishOnlyWhenRetained = false): Promise<void> {
  clearPollTimer();
  const generation = requestGeneration;
  const requestId = ++nextRequestId;
  activeRequestId = requestId;
  const request = (async () => {
    try {
      const approvals = await fetchPendingApprovals();
      if (
        generation !== requestGeneration ||
        (publishOnlyWhenRetained && retainedIntervals.length === 0)
      ) {
        return;
      }
      emit(makeSnapshot(approvals, null, retainedIntervals.length > 0));
      log('refresh succeeded approval_count=%d', approvals.length);
    } catch (error) {
      if (
        generation !== requestGeneration ||
        (publishOnlyWhenRetained && retainedIntervals.length === 0)
      ) {
        return;
      }
      emit(makeSnapshot(snapshot.approvals, normalizeError(error), retainedIntervals.length > 0));
      const errorType = error instanceof Error ? error.name : typeof error;
      log('refresh failed error_type=%s', errorType);
    } finally {
      if (activeRequestId === requestId) {
        activeRequestId = null;
        inFlight = null;
      }
      scheduleNextPoll(generation);
    }
  })();
  inFlight = request;
  return request;
}

export function refreshFlowPendingApprovals(): Promise<void> {
  if (!inFlight) return startRefresh();
  if (queuedRefresh) return queuedRefresh;

  clearPollTimer();
  const generation = requestGeneration;
  const activeRequest = inFlight;
  const queuedWhileRetained = retainedIntervals.length > 0;
  const queued: Promise<void> = activeRequest.then(() => {
    if (queuedRefresh === queued) queuedRefresh = null;
    if (
      generation !== requestGeneration ||
      (queuedWhileRetained && retainedIntervals.length === 0)
    ) {
      return;
    }
    return startRefresh();
  });
  queuedRefresh = queued;
  return queued;
}

export function retainFlowPendingApprovalsPolling(
  intervalMs: number = POLL_INTERVAL_MS
): () => void {
  const previousInterval = effectiveIntervalMs();
  retainedIntervals.push(intervalMs);
  if (retainedIntervals.length === 1) {
    emit(makeSnapshot(snapshot.approvals, snapshot.error, true));
    if (!inFlight) void startRefresh(true);
  } else if (intervalMs < previousInterval) {
    // A faster consumer joined an already-scheduled slow poll. Re-arm, or it
    // would wait out the slow timer before honouring the new cadence.
    clearPollTimer();
    scheduleNextPoll(requestGeneration);
  }

  let released = false;
  return () => {
    if (released) return;
    released = true;
    const previous = effectiveIntervalMs();
    const index = retainedIntervals.indexOf(intervalMs);
    if (index !== -1) retainedIntervals.splice(index, 1);
    if (retainedIntervals.length > 0) {
      // Symmetric with the join branch above: the departing consumer may have
      // been the fast one, leaving its short timer armed on behalf of
      // consumers that asked for less. Re-arm at the cadence that is now
      // current, or the queue keeps polling fast for one more tick after the
      // flow inspector that wanted it has closed.
      if (effectiveIntervalMs() > previous) {
        clearPollTimer();
        scheduleNextPoll(requestGeneration);
      }
      return;
    }

    clearPollTimer();
    emit(makeSnapshot(snapshot.approvals, snapshot.error, false));
  };
}

export function useFlowPendingApprovalsSource(
  enabled: boolean,
  intervalMs: number = POLL_INTERVAL_MS
): FlowPendingApprovalsSnapshot {
  const subscribe = useCallback(
    (listener: () => void) => (enabled ? subscribeFlowPendingApprovals(listener) : () => undefined),
    [enabled]
  );
  const getSnapshot = useCallback(
    () => (enabled ? getFlowPendingApprovalsSnapshot() : INITIAL_SNAPSHOT),
    [enabled]
  );
  const current = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);

  useEffect(() => {
    if (!enabled) return;
    return retainFlowPendingApprovalsPolling(intervalMs);
  }, [enabled, intervalMs]);

  return current;
}

export function resetFlowPendingApprovalsStoreForTests(): void {
  clearPollTimer();
  requestGeneration += 1;
  activeRequestId = null;
  retainedIntervals = [];
  inFlight = null;
  queuedRefresh = null;
  snapshot = INITIAL_SNAPSHOT;
  listeners.clear();
}
