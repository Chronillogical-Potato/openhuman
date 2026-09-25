import { describe, expect, it } from 'vitest';

import reducer, { beginInferenceTurn, markInferenceTurnStreaming } from '../chatRuntimeSlice';

/**
 * The Playwright driver dispatches `beginInferenceTurn` as a RAW ACTION OBJECT.
 *
 * `app/test/playwright/helpers/chat-drive.ts` (`armTurnLifecycle`) cannot import
 * the slice's action creator — it runs inside the page, through
 * `page.evaluate`, with only `window.__OPENHUMAN_STORE__` to work with. So it
 * writes the action type as the literal string `'chatRuntime/beginInferenceTurn'`.
 *
 * Redux silently ignores an action whose type matches no reducer. If the slice
 * were renamed, or this reducer renamed, that literal would become a no-op and
 * every Playwright surface gated on `s.thread.isRunning` would quietly stop
 * being observable again — which is the exact failure the driver change fixed,
 * so it would look like a regression in the product rather than in the driver.
 *
 * These tests are the pin. They live with the slice, not with the spec, because
 * the thing that can break is a rename here.
 */
describe('chatRuntimeSlice — turn lifecycle wire contract (Playwright driver)', () => {
  /** Kept byte-identical to the literal in `chat-drive.ts`'s `armTurnLifecycle`. */
  const DRIVER_ACTION_TYPE = 'chatRuntime/beginInferenceTurn';

  it('beginInferenceTurn keeps the action type the Playwright driver hardcodes', () => {
    expect(beginInferenceTurn.type).toBe(DRIVER_ACTION_TYPE);
  });

  it('a raw action object with that type creates the lifecycle entry', () => {
    // Deliberately NOT the action creator: the creator working proves nothing
    // about the string the driver actually sends.
    const next = reducer(undefined, {
      type: DRIVER_ACTION_TYPE,
      payload: { threadId: 'thread-1' },
    });

    expect(next.inferenceTurnLifecycleByThread['thread-1']).toBe('started');
  });

  it('markInferenceTurnStreaming alone cannot arm a thread, which is why the driver must', () => {
    // The reason `armTurnLifecycle` exists. `ChatRuntimeProvider` dispatches
    // this on the socket's `inference_start`; against a thread with no entry it
    // is a no-op, so an RPC-driven turn would never become `isRunning`.
    const withoutBegin = reducer(undefined, markInferenceTurnStreaming({ threadId: 'thread-1' }));
    expect(withoutBegin.inferenceTurnLifecycleByThread['thread-1']).toBeUndefined();

    const armed = reducer(undefined, beginInferenceTurn({ threadId: 'thread-1' }));
    const streaming = reducer(armed, markInferenceTurnStreaming({ threadId: 'thread-1' }));
    expect(streaming.inferenceTurnLifecycleByThread['thread-1']).toBe('streaming');
  });
});
