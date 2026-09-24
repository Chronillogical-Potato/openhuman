/**
 * Coalesce streamed text/thinking deltas into at most one store update per
 * animation frame.
 *
 * Every socket delta used to be its own Redux dispatch, and every dispatch
 * re-projected and re-rendered the live assistant tail — a token rate well
 * above the display's frame rate spent most of that work on frames nobody saw,
 * and that backlog is what the stream stuttered on. A frame is the natural
 * unit: nothing is painted between two of them anyway.
 *
 * ## Ordering is preserved, not traded away
 *
 * - Only ADJACENT deltas for the same `(thread, request, round, channel)` are
 *   merged. A delta for a different key closes the current run, so interleaved
 *   thinking and content — or two threads streaming at once — replay in the
 *   exact order they arrived.
 * - Every other chat event must call {@link ChatDeltaCoalescer.flush} before it
 *   is handled. A `tool_call` therefore always lands after the text that
 *   preceded it, which is the order the transcript records (and the order the
 *   parts render in).
 */

export type DeltaChannel = 'content' | 'thinking';

export interface CoalescibleDelta {
  thread_id: string;
  request_id: string;
  round: number;
  delta: string;
}

type Pending<E extends CoalescibleDelta> = { channel: DeltaChannel; event: E };

/** Schedules one flush; returns a canceller. */
export type FlushScheduler = (flush: () => void) => () => void;

/**
 * The next animation frame, or a short timer when there is no frame to wait
 * for (a hidden window throttles rAF to nothing, and the stream must not stall
 * there — it would all land at once on refocus).
 */
export const frameScheduler: FlushScheduler = flush => {
  let done = false;
  const run = () => {
    if (done) return;
    done = true;
    flush();
  };
  const raf = typeof requestAnimationFrame === 'function' ? requestAnimationFrame(run) : undefined;
  const timer = setTimeout(run, 50);
  return () => {
    done = true;
    if (raf !== undefined && typeof cancelAnimationFrame === 'function') cancelAnimationFrame(raf);
    clearTimeout(timer);
  };
};

export interface ChatDeltaCoalescer<E extends CoalescibleDelta> {
  /** Queue a delta; merged into the previous one when the key matches. */
  push: (channel: DeltaChannel, event: E) => void;
  /** Deliver everything queued, in arrival order, now. */
  flush: () => void;
  /** Drop the scheduled flush (queued deltas are delivered first). */
  dispose: () => void;
}

export function createChatDeltaCoalescer<E extends CoalescibleDelta>(
  deliver: (channel: DeltaChannel, event: E) => void,
  schedule: FlushScheduler = frameScheduler
): ChatDeltaCoalescer<E> {
  let queue: Pending<E>[] = [];
  let cancel: (() => void) | null = null;

  const flush = () => {
    cancel?.();
    cancel = null;
    if (queue.length === 0) return;
    const drained = queue;
    queue = [];
    for (const item of drained) deliver(item.channel, item.event);
  };

  const push = (channel: DeltaChannel, event: E) => {
    const last = queue[queue.length - 1];
    if (
      last &&
      last.channel === channel &&
      last.event.thread_id === event.thread_id &&
      last.event.request_id === event.request_id &&
      last.event.round === event.round
    ) {
      last.event = { ...last.event, delta: `${last.event.delta}${event.delta}` };
    } else {
      queue.push({ channel, event });
    }
    cancel ??= schedule(flush);
  };

  return { push, flush, dispose: flush };
}

type DeltaListeners<E extends CoalescibleDelta> = {
  onTextDelta?: (event: E) => void;
  onThinkingDelta?: (event: E) => void;
};

/**
 * Wrap a chat listener set so text/thinking deltas are coalesced per frame and
 * every other listener flushes them first (see the ordering note above).
 * `dispose` delivers anything still queued and stops the scheduled flush.
 */
export function withCoalescedDeltas<E extends CoalescibleDelta, L extends DeltaListeners<E>>(
  listeners: L,
  schedule: FlushScheduler = frameScheduler
): { listeners: L; dispose: () => void } {
  const coalescer = createChatDeltaCoalescer<E>((channel, event) => {
    if (channel === 'content') listeners.onTextDelta?.(event);
    else listeners.onThinkingDelta?.(event);
  }, schedule);
  const wrapped: Record<string, unknown> = {};
  for (const [name, listener] of Object.entries(listeners as Record<string, unknown>)) {
    if (typeof listener !== 'function') {
      wrapped[name] = listener;
      continue;
    }
    if (name === 'onTextDelta') {
      wrapped[name] = (event: E) => coalescer.push('content', event);
    } else if (name === 'onThinkingDelta') {
      wrapped[name] = (event: E) => coalescer.push('thinking', event);
    } else {
      wrapped[name] = (...args: unknown[]) => {
        coalescer.flush();
        return (listener as (...a: unknown[]) => unknown)(...args);
      };
    }
  }
  return { listeners: wrapped as L, dispose: coalescer.dispose };
}
