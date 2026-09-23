/**
 * Sub-agent spend reaches the composer exactly ONCE, whichever way it was
 * spawned (#6459).
 *
 * The composer's totals are already sub-agent-inclusive: `applyTurnUsage`
 * folds the top-level figures, which `chat_done` sends as parent+child. The
 * defect is that a DETACHED child's spend never arrives — `detached_child()`
 * severs `parent_subagent_usage`, so `holistic_last_turn_usage` folds nothing
 * and `chat_done` reports parent-only.
 *
 * Fixing that creates the opposite risk, which is worse: a BLOCKING child's
 * spend is already inside `chat_done` (tokens **and** `charged_amount_usd`),
 * so adding it again would double the user's reported money. A doubled cost
 * reads as fact and gets quoted; a missing one reads as "not implemented".
 *
 * So the assertion here is not "sub-agent tokens appear" — that passes the
 * moment the field exists. It is **once, in both modes, tokens and cost**.
 */
import { describe, expect, it } from 'vitest';

import chatRuntimeReducer, { recordChatTurnUsage } from '../chatRuntimeSlice';

const THREAD = 't-spend';

function reduce(actions: ReturnType<typeof recordChatTurnUsage>[]) {
  let state = chatRuntimeReducer(undefined, { type: '@@INIT' });
  for (const action of actions) state = chatRuntimeReducer(state, action);
  return state.usageByThread[THREAD];
}

/** `chat_done` for a BLOCKING spawn: top-level totals already include the child. */
const blockingChatDone = recordChatTurnUsage({
  threadId: THREAD,
  inputTokens: 1000, // 700 parent + 300 child
  outputTokens: 200, // 150 parent + 50 child
  costUsd: 0.05, // parent + child
  contextWindow: 100_000,
  subAgents: [{ agentId: 'researcher', inputTokens: 300, outputTokens: 50, costUsd: 0.02 }],
});

/** `chat_done` for a DETACHED spawn: parent-only, because the ledger was severed. */
const detachedChatDone = recordChatTurnUsage({
  threadId: THREAD,
  inputTokens: 700,
  outputTokens: 150,
  costUsd: 0.03,
  contextWindow: 100_000,
});

/** The late `subagent_completed` delta the detached child now sends. */
const detachedChildSpend = recordChatTurnUsage({
  threadId: THREAD,
  inputTokens: 300,
  outputTokens: 50,
  costUsd: 0.02,
  subAgentSpendOnly: true,
  subAgents: [{ agentId: 'researcher', inputTokens: 300, outputTokens: 50, costUsd: 0.02 }],
});

describe('sub-agent spend reaches the composer once', () => {
  it('counts a blocking child once — its spend is already in chat_done', () => {
    const usage = reduce([blockingChatDone]);

    expect(usage?.inputTokens).toBe(1000);
    expect(usage?.outputTokens).toBe(200);
    expect(usage?.costUsd).toBeCloseTo(0.05, 6);
  });

  it('counts a detached child once — its spend arrives separately', () => {
    // Same real-world turn as the blocking case: 700+300 in, 150+50 out,
    // $0.03+$0.02. The totals must land identically however it was spawned.
    const usage = reduce([detachedChatDone, detachedChildSpend]);

    expect(usage?.inputTokens).toBe(1000);
    expect(usage?.outputTokens).toBe(200);
    expect(usage?.costUsd).toBeCloseTo(0.05, 6);
  });

  it('does NOT double a blocking child if a late delta also arrives', () => {
    // The failure this guard exists for. The core must not populate usage on
    // `subagent_completed` for a blocking spawn; if it ever did, this is what
    // the user would see. Asserted so the expected number is written down.
    const usage = reduce([blockingChatDone, detachedChildSpend]);

    expect(usage?.inputTokens).toBe(1300);
    expect(usage?.costUsd).toBeCloseTo(0.07, 6);
  });

  it('does not count a sub-agent delta as another turn', () => {
    const blocking = reduce([blockingChatDone]);
    const detached = reduce([detachedChatDone, detachedChildSpend]);

    expect(blocking?.turns).toBe(1);
    // One user turn happened, not two — the child is more spend on the same
    // turn, and counting it skews every per-turn average derived from this.
    expect(detached?.turns).toBe(1);
  });

  it('leaves the context gauge alone when a sub-agent delta lands', () => {
    // `lastTurnContextUsed` is the orchestrator's own window occupancy and
    // deliberately excludes children (#4271). Recomputing it from a delta whose
    // top-level and sub-agent figures are the same numbers yields
    // `max(0, 300+50-350) = 0`, blanking the bar the parent's turn just set.
    const detached = reduce([detachedChatDone, detachedChildSpend]);

    expect(detached?.lastTurnContextUsed).toBe(850);
  });

  it('records the per-agent breakdown once in both modes', () => {
    const blocking = reduce([blockingChatDone]);
    const detached = reduce([detachedChatDone, detachedChildSpend]);

    expect(blocking?.subAgents.researcher).toMatchObject({ inputTokens: 300, runs: 1 });
    expect(detached?.subAgents.researcher).toMatchObject({ inputTokens: 300, runs: 1 });
  });
});
