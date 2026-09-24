/**
 * Regression guard: a streamed turn and the same turn reopened from history
 * must render the same way, and the stream must never reshape what is already
 * on screen.
 *
 * The defect this pins: a live thread glitched — tools reordered, text
 * restarted its reveal, cards collapsed, the view jumped — and the same thread
 * opened fresh looked right. Each glitch was a part that moved or remounted,
 * because assistant-ui keys text/reasoning parts and whole messages by INDEX.
 * The properties below are the ones whose violation produced those glitches:
 *
 * 1. **Parity** — the settled message's parts equal the parts the core
 *    projection (`mapDisplayItems`) produces for the same turn on reload.
 * 2. **Append-only** — while the turn streams, no part that is on screen moves
 *    to another index or changes kind/id; text only grows.
 * 3. **Atomic settle** — across every store update of `chat_done`, the turn
 *    stays at one message index, never renders twice, never loses its answer,
 *    and the swap from live tail to persisted reply keeps every part in place.
 *
 * Both tellings of the turn come from one fixture (`test/fixtures/streamedTurn`),
 * driven through the real socket handlers of `ChatRuntimeProvider`.
 */
import type { ThreadMessageLike } from '@assistant-ui/react';
import { render, waitFor } from '@testing-library/react';
import { act } from 'react';
import { Provider } from 'react-redux';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import * as chatService from '../../services/chatService';
import { mapDisplayItems } from '../../features/conversations/derived/mapDisplayItems';
import { threadApi } from '../../services/api/threadApi';
import { store } from '../../store';
import { beginInferenceTurn, clearAllChatRuntime } from '../../store/chatRuntimeSlice';
import { setStatusForUser } from '../../store/socketSlice';
import { addMessageLocal, clearAllThreads } from '../../store/threadSlice';
import {
  DONE_EVENT,
  FINAL_ANSWER,
  HISTORY_ITEMS,
  LIVE_TURN_STEPS,
  type SocketStep,
  TURN_REQUEST,
  TURN_THREAD,
} from '../../test/fixtures/streamedTurn';
import type { ThreadMessage } from '../../types/thread';
import { buildRuntimeMessages, STREAMING_TAIL_ID } from '../assistantUiMessages';
import ChatRuntimeProvider from '../ChatRuntimeProvider';

vi.mock('../../services/chatService', async () => {
  const actual = await vi.importActual<typeof chatService>('../../services/chatService');
  return { ...actual, subscribeChatEvents: vi.fn() };
});

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    createNewThread: vi.fn(),
    getThreads: vi.fn(),
    getThreadMessages: vi.fn(),
    appendMessage: vi.fn(),
    generateTitleIfNeeded: vi.fn(),
    updateMessage: vi.fn(),
    deleteThread: vi.fn(),
    purge: vi.fn(),
    getTurnState: vi.fn(),
    listRuns: vi.fn(),
  },
}));

vi.mock('../../services/socketService', () => ({
  socketService: {
    subscribeThread: vi.fn(() => Promise.resolve(true)),
    on: vi.fn(),
    off: vi.fn(),
  },
}));

vi.mock('../../hooks/usageRefresh', () => ({ requestUsageRefresh: vi.fn() }));

vi.mock('../../hooks/useRefetchSnapshotOnTurnEnd', () => ({
  useRefetchSnapshotOnTurnEnd: () => ({ refetch: vi.fn() }),
}));

const USER_MESSAGE: ThreadMessage = {
  id: 'user-1',
  content: 'What is on today?',
  type: 'text',
  extraMetadata: {},
  sender: 'user',
  createdAt: '2026-09-24T10:00:00.000Z',
};

function renderProvider(): chatService.ChatEventListeners {
  let captured: chatService.ChatEventListeners = {};
  vi.mocked(chatService.subscribeChatEvents).mockImplementation(listeners => {
    captured = listeners;
    return () => {};
  });
  store.dispatch(setStatusForUser({ userId: '__pending__', status: 'connected' }));
  render(
    <Provider store={store}>
      <ChatRuntimeProvider>
        <div />
      </ChatRuntimeProvider>
    </Provider>
  );
  return captured;
}

function fire(listeners: chatService.ChatEventListeners, step: SocketStep) {
  const listener = listeners[step.listener] as ((event: unknown) => void) | undefined;
  if (!listener) throw new Error(`provider does not handle ${step.listener}`);
  act(() => listener(step.event));
}

/**
 * The runtime messages for the thread, built from the store exactly as
 * `useOpenHumanExternalStore` builds them (minus the core-transcript fetch,
 * which a settled turn of this session does not read — its frozen trail wins).
 */
function project(): ThreadMessageLike[] {
  const state = store.getState();
  const runtime = state.chatRuntime;
  const streaming = runtime.streamingAssistantByThread[TURN_THREAD] ?? null;
  const lifecycle = runtime.inferenceTurnLifecycleByThread[TURN_THREAD];
  return buildRuntimeMessages(state.thread.messagesByThreadId[TURN_THREAD] ?? [], streaming, {
    isRunning: lifecycle === 'started' || lifecycle === 'streaming',
    liveTimeline: runtime.toolTimelineByThread[TURN_THREAD] ?? [],
    liveTranscript: runtime.processingByThread[TURN_THREAD] ?? [],
    pendingApproval: runtime.pendingApprovalByThread[TURN_THREAD] ?? null,
    settledTurns: runtime.settledTurnsByThread?.[TURN_THREAD] ?? {},
    liveRequestId: runtime.liveRequestIdByThread?.[TURN_THREAD] ?? streaming?.requestId,
  });
}

type PartShape =
  | { type: 'text' | 'reasoning'; text: string }
  | { type: 'tool-call'; toolName: string; args?: unknown; settled: boolean }
  | { type: string };

/**
 * What a reader sees of a part. Tool ids are compared separately where they
 * must match; here a sub-agent's id legitimately differs between the socket
 * (`<thread>:subagent:<task>:<agent>`) and the core (`subagent:<agent>`),
 * which is exactly why a settled turn keeps its frozen live trail in-session.
 */
function shape(parts: ThreadMessageLike['content']): PartShape[] {
  if (typeof parts === 'string') return [{ type: 'text', text: parts }];
  return parts.map(part => {
    if (part.type === 'text' || part.type === 'reasoning') {
      return { type: part.type, text: part.text };
    }
    if (part.type === 'tool-call') {
      return {
        type: 'tool-call',
        toolName: part.toolName,
        // A delegation's args carry live progress while it runs; compare the
        // plain calls' input only.
        ...(part.toolName === 'task' ? {} : { args: part.args }),
        settled: part.result !== undefined,
      };
    }
    return { type: part.type };
  });
}

function partsOf(message: ThreadMessageLike | undefined): ThreadMessageLike['content'] {
  if (!message) throw new Error('message missing');
  return message.content;
}

function arrayParts(message: ThreadMessageLike | undefined) {
  const parts = partsOf(message);
  if (typeof parts === 'string') throw new Error('expected part array');
  return parts;
}

/** The key assistant-ui gives a part: tool id for tool calls, index otherwise. */
function partKey(part: ReturnType<typeof arrayParts>[number], index: number): string {
  return part.type === 'tool-call' ? `tool:${part.toolCallId}` : `${part.type}@${index}`;
}

describe('live ≡ history rendering of one turn', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    store.dispatch(clearAllThreads());
    store.dispatch(clearAllChatRuntime());
    vi.mocked(threadApi.appendMessage).mockImplementation(async (_tid, message) => message);
    vi.mocked(threadApi.getThreads).mockResolvedValue({ threads: [], count: 0 });
    vi.mocked(threadApi.getTurnState).mockResolvedValue(null);
    vi.mocked(threadApi.listRuns).mockResolvedValue([]);
    vi.mocked(threadApi.generateTitleIfNeeded).mockResolvedValue({
      id: TURN_THREAD,
      title: 'Today',
    } as never);
  });

  afterEach(() => {
    store.dispatch(setStatusForUser({ userId: '__pending__', status: 'disconnected' }));
  });

  async function startTurn() {
    await store.dispatch(addMessageLocal({ threadId: TURN_THREAD, message: USER_MESSAGE }));
    store.dispatch(beginInferenceTurn({ threadId: TURN_THREAD }));
  }

  /** Stream the whole fixture, recording the tail's parts after every event. */
  async function streamTurn(listeners: chatService.ChatEventListeners) {
    const snapshots: ReturnType<typeof arrayParts>[] = [];
    for (const step of LIVE_TURN_STEPS) {
      fire(listeners, step);
      // Deltas are coalesced per frame; the next non-delta event flushes them,
      // and the last run is flushed by waiting for the frame.
      const tail = project().at(-1);
      if (tail?.id === STREAMING_TAIL_ID) snapshots.push(arrayParts(tail));
    }
    await waitFor(() => {
      const tail = project().at(-1);
      expect(tail?.id).toBe(STREAMING_TAIL_ID);
      expect(shape(partsOf(tail)).at(-1)).toEqual({ type: 'text', text: FINAL_ANSWER });
    });
    snapshots.push(arrayParts(project().at(-1)));
    return snapshots;
  }

  it('streams append-only: nothing on screen moves, changes kind, or loses text', async () => {
    const listeners = renderProvider();
    await startTurn();
    const snapshots = await streamTurn(listeners);

    expect(snapshots.length).toBeGreaterThan(5);
    for (let at = 1; at < snapshots.length; at += 1) {
      const before = snapshots[at - 1];
      const after = snapshots[at];
      expect(after.length).toBeGreaterThanOrEqual(before.length);
      before.forEach((part, index) => {
        const next = after[index];
        // The one sanctioned change: `subagent_spawned` promotes the
        // `spawn_subagent` call, in its own slot, into the delegation card.
        // It becomes a different component either way; what matters is that
        // it stays where the agent issued it and nothing around it moves.
        const promoted =
          part.type === 'tool-call' &&
          part.toolName === 'spawn_subagent' &&
          next.type === 'tool-call' &&
          next.toolName === 'task';
        if (promoted) return;
        // Same key at the same index — what keeps React from remounting it.
        expect(partKey(next, index)).toBe(partKey(part, index));
        if ((part.type === 'text' || part.type === 'reasoning') && next.type === part.type) {
          expect(next.text.startsWith(part.text)).toBe(true);
        }
      });
    }
  });

  it('settles into exactly what the core projection renders on reload', async () => {
    const listeners = renderProvider();
    await startTurn();
    await streamTurn(listeners);

    act(() => listeners.onDone?.(DONE_EVENT));
    await waitFor(() =>
      expect(store.getState().chatRuntime.inferenceTurnLifecycleByThread[TURN_THREAD]).toBe(
        undefined
      )
    );

    const settled = project();
    expect(settled.map(message => message.role)).toEqual(['user', 'assistant']);
    const settledAnswer = settled[1];
    expect(settledAnswer?.id).not.toBe(STREAMING_TAIL_ID);

    // The same turn, reopened: the persisted reply plus the core projection.
    const reloadedMessages = store.getState().thread.messagesByThreadId[TURN_THREAD] ?? [];
    const history = mapDisplayItems(HISTORY_ITEMS);
    const reloaded = buildRuntimeMessages(reloadedMessages, null, {
      isRunning: false,
      turnTimelines: history.timelines,
      turnTranscripts: history.transcripts,
    });

    expect(shape(partsOf(settledAnswer))).toEqual(shape(partsOf(reloaded[1])));
    // And that shape is the turn as it happened, narration included.
    expect(shape(partsOf(reloaded[1])).map(part => part.type)).toEqual([
      'reasoning',
      'text',
      'tool-call',
      'tool-call',
      'reasoning',
      'tool-call',
      'text',
    ]);
  });

  it('settles in one store update: same index, same parts, never doubled, never blank', async () => {
    const listeners = renderProvider();
    await startTurn();
    await streamTurn(listeners);

    const lastTail = arrayParts(project().at(-1));
    const turnIndex = project().length - 1;

    // Record the projection at EVERY store notification through `chat_done`.
    const seen: ThreadMessageLike[][] = [];
    const unsubscribe = store.subscribe(() => seen.push(project()));
    try {
      act(() => listeners.onDone?.(DONE_EVENT));
      await waitFor(() =>
        expect(store.getState().chatRuntime.inferenceTurnLifecycleByThread[TURN_THREAD]).toBe(
          undefined
        )
      );
    } finally {
      unsubscribe();
    }

    expect(seen.length).toBeGreaterThan(0);
    let swaps = 0;
    let previousId = STREAMING_TAIL_ID;
    for (const messages of seen) {
      const assistants = messages.filter(message => message.role === 'assistant');
      // Never the reply AND the tail at once.
      expect(assistants).toHaveLength(1);
      // Always at the tail's index — a shift is a remount.
      expect(messages.length - 1).toBe(turnIndex);
      const turn = messages[turnIndex];
      const parts = arrayParts(turn);
      // Never blank: the answer is on screen in every intermediate state.
      expect(shape(parts).at(-1)).toEqual({ type: 'text', text: FINAL_ANSWER });
      // Every part stays at its index under its key.
      expect(parts.map(partKey)).toEqual(lastTail.map(partKey));
      if (turn?.id !== previousId) swaps += 1;
      previousId = turn?.id ?? previousId;
    }
    // Exactly one transition from the live tail to the persisted reply.
    expect(swaps).toBe(1);
    expect(previousId).not.toBe(STREAMING_TAIL_ID);
    expect(
      store.getState().chatRuntime.settledTurnsByThread[TURN_THREAD]?.[TURN_REQUEST]
    ).toBeDefined();
  });

  it('keeps the settled turn on its frozen trail when the core projection arrives', async () => {
    const listeners = renderProvider();
    await startTurn();
    await streamTurn(listeners);
    act(() => listeners.onDone?.(DONE_EVENT));
    await waitFor(() =>
      expect(store.getState().chatRuntime.inferenceTurnLifecycleByThread[TURN_THREAD]).toBe(
        undefined
      )
    );

    const before = arrayParts(project()[1]);
    // The core projection lands with its own ids for the same turn.
    const history = mapDisplayItems(HISTORY_ITEMS);
    const runtime = store.getState().chatRuntime;
    const after = arrayParts(
      buildRuntimeMessages(store.getState().thread.messagesByThreadId[TURN_THREAD] ?? [], null, {
        isRunning: false,
        turnTimelines: history.timelines,
        turnTranscripts: history.transcripts,
        settledTurns: runtime.settledTurnsByThread[TURN_THREAD] ?? {},
      })[1]
    );
    // Same keys: nothing remounts when the projection replaces nothing.
    expect(after.map(partKey)).toEqual(before.map(partKey));
  });
});
