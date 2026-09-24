/**
 * Plan/build run mode for a thread.
 *
 * The live value is kept in `runModeSlice`, updated by the `run_mode_changed`
 * socket event (wired centrally in `ChatRuntimeProvider`, same pattern as
 * {@link useThreadTodos} / {@link useThreadGoal}). This hook additionally
 * primes the slice on thread open via `openhuman.agent_get_run_mode` when no
 * entry exists yet, and exposes `setMode` to flip it via
 * `openhuman.agent_set_run_mode`.
 *
 * Not yet backed by real core behavior: both RPCs and `run_mode_changed` are
 * coded to the WS-C plan-mode contract but unimplemented by the core as of
 * this writing — `setMode` will reject until that lands.
 */
import debug from 'debug';
import { useCallback, useEffect, useRef } from 'react';

import { callCoreRpc } from '../../../services/coreRpcClient';
import { store } from '../../../store';
import { useAppDispatch, useAppSelector } from '../../../store/hooks';
import { type RunMode, setRunMode } from '../../../store/runModeSlice';

const log = debug('openhuman:chat:run-mode');

const DEFAULT_MODE: RunMode = 'build';

export interface UseRunModeResult {
  mode: RunMode;
  setMode: (mode: RunMode) => Promise<void>;
}

export function useRunMode(threadId: string | null): UseRunModeResult {
  const dispatch = useAppDispatch();
  const mode = useAppSelector(state =>
    threadId ? state.runMode.byThread[threadId] ?? DEFAULT_MODE : DEFAULT_MODE
  );
  const loadedFor = useRef<string | null>(null);

  useEffect(() => {
    if (!threadId || loadedFor.current === threadId) return;
    loadedFor.current = threadId;
    // Only fetch when the slice has no live entry yet — a value already set
    // (e.g. by a `run_mode_changed` event that arrived first) wins.
    if (store.getState().runMode.byThread[threadId] !== undefined) return;
    let cancelled = false;
    void (async () => {
      try {
        const response = await callCoreRpc<{ data?: { mode?: RunMode } }>({
          method: 'openhuman.agent_get_run_mode',
          params: { thread_id: threadId },
        });
        if (cancelled) return;
        const fetchedMode =
          response && typeof response === 'object' && 'data' in response
            ? (response as { data?: { mode?: RunMode } }).data?.mode
            : (response as { mode?: RunMode } | undefined)?.mode;
        if (fetchedMode === 'plan' || fetchedMode === 'build') {
          dispatch(setRunMode({ threadId, mode: fetchedMode }));
        }
      } catch (e) {
        log('agent_get_run_mode failed (core may not support it yet): %o', e);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [threadId, dispatch]);

  const setMode = useCallback(
    async (nextMode: RunMode) => {
      if (!threadId) return;
      // Optimistic: the `run_mode_changed` event (or a failure) reconciles it.
      dispatch(setRunMode({ threadId, mode: nextMode }));
      try {
        await callCoreRpc({
          method: 'openhuman.agent_set_run_mode',
          params: { thread_id: threadId, mode: nextMode },
        });
      } catch (e) {
        log('agent_set_run_mode failed: %o', e);
        throw e;
      }
    },
    [threadId, dispatch]
  );

  return { mode, setMode };
}
