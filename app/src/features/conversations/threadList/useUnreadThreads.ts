import { useEffect, useRef, useState } from 'react';

/**
 * Tracks which threads finished a reply while the user was looking elsewhere.
 *
 * A thread becomes unread on the running → idle edge when it is not the
 * selected thread, and is cleared the moment it is selected. Runtime-only:
 * the core has no read receipts, so this resets on reload rather than
 * inventing a persisted "seen" state the backend cannot confirm.
 *
 * `runningThreadIds` must be a stable-per-content key (sorted ids joined) so
 * the effect only runs when the running set actually changes.
 */
export function useUnreadThreads(
  runningThreadIds: readonly string[],
  selectedThreadId: string | null
): ReadonlySet<string> {
  const [unread, setUnread] = useState<ReadonlySet<string>>(() => new Set());
  const previousRunning = useRef<ReadonlySet<string>>(new Set());
  const runningKey = runningThreadIds.join('\u0000');

  useEffect(() => {
    const running = new Set(runningThreadIds);
    const finished = [...previousRunning.current].filter(
      id => !running.has(id) && id !== selectedThreadId
    );
    previousRunning.current = running;
    if (finished.length === 0) return;
    setUnread(prev => {
      const next = new Set(prev);
      for (const id of finished) next.add(id);
      return next;
    });
    // `runningKey` carries the content of `runningThreadIds`.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runningKey, selectedThreadId]);

  useEffect(() => {
    if (!selectedThreadId) return;
    setUnread(prev => {
      if (!prev.has(selectedThreadId)) return prev;
      const next = new Set(prev);
      next.delete(selectedThreadId);
      return next;
    });
  }, [selectedThreadId]);

  return unread;
}
