/**
 * The shared pending-approvals poller now serves consumers with different
 * cadences, and must run at the fastest one anybody asked for.
 *
 * Before the background-approvals surface every consumer wanted 2s, because
 * every consumer was watching a flow run the user had just started. The
 * background surface is mounted for the whole session and only needs to notice
 * a park with a 600s TTL, so polling an almost-always-empty table every 2s for
 * the life of the app would be cost with no benefit. But a slow consumer must
 * never be able to slow down a flow run the user IS watching.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { fetchPendingApprovals } from '../../services/api/approvalApi';
import {
  resetFlowPendingApprovalsStoreForTests,
  retainFlowPendingApprovalsPolling,
} from '../flowPendingApprovalsStore';

vi.mock('../../services/api/approvalApi', async importOriginal => {
  const actual = await importOriginal<typeof import('../../services/api/approvalApi')>();
  return { ...actual, fetchPendingApprovals: vi.fn() };
});

/** Let the in-flight fetch settle so `scheduleNextPoll` has run. */
const settle = () => vi.advanceTimersByTimeAsync(0);

beforeEach(() => {
  vi.useFakeTimers();
  resetFlowPendingApprovalsStoreForTests();
  vi.mocked(fetchPendingApprovals).mockResolvedValue([]);
});

afterEach(() => {
  resetFlowPendingApprovalsStoreForTests();
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe('poll cadence across consumers', () => {
  it('polls at the slow cadence when only a slow consumer is retained', async () => {
    const release = retainFlowPendingApprovalsPolling(15_000);
    await settle();
    const afterFirst = vi.mocked(fetchPendingApprovals).mock.calls.length;

    await vi.advanceTimersByTimeAsync(2_000);
    expect(vi.mocked(fetchPendingApprovals).mock.calls.length).toBe(afterFirst);

    await vi.advanceTimersByTimeAsync(13_000);
    expect(vi.mocked(fetchPendingApprovals).mock.calls.length).toBeGreaterThan(afterFirst);
    release();
  });

  it('speeds up immediately when a fast consumer joins a slow poll', async () => {
    // THE branch that is easy to get wrong: the slow timer is already armed, so
    // without re-arming, a flow inspector opening would wait out the remaining
    // 15s before its first 2s tick — the user watching a run would see it stall.
    const releaseSlow = retainFlowPendingApprovalsPolling(15_000);
    await settle();
    const beforeFast = vi.mocked(fetchPendingApprovals).mock.calls.length;

    const releaseFast = retainFlowPendingApprovalsPolling(2_000);
    await vi.advanceTimersByTimeAsync(2_000);

    expect(vi.mocked(fetchPendingApprovals).mock.calls.length).toBeGreaterThan(beforeFast);
    releaseFast();
    releaseSlow();
  });

  it('returns to the slow cadence when the fast consumer leaves', async () => {
    const releaseSlow = retainFlowPendingApprovalsPolling(15_000);
    const releaseFast = retainFlowPendingApprovalsPolling(2_000);
    await settle();
    releaseFast();
    await settle();
    const afterRelease = vi.mocked(fetchPendingApprovals).mock.calls.length;

    await vi.advanceTimersByTimeAsync(2_000);
    expect(vi.mocked(fetchPendingApprovals).mock.calls.length).toBe(afterRelease);
    releaseSlow();
  });

  it('stops polling once every consumer has released', async () => {
    const release = retainFlowPendingApprovalsPolling(2_000);
    await settle();
    release();
    const afterRelease = vi.mocked(fetchPendingApprovals).mock.calls.length;

    await vi.advanceTimersByTimeAsync(60_000);
    expect(vi.mocked(fetchPendingApprovals).mock.calls.length).toBe(afterRelease);
  });
});
