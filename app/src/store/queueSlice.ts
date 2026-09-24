/**
 * The core's run queue, per thread, as the composer's message queue renders it.
 *
 * Two lists with two owners:
 *
 * - `itemsByThread` is the core's: messages sitting in a running turn's queue,
 *   filled and drained by the `queue_item_queued` / `queue_item_delivered` /
 *   `queue_item_removed` socket events. It is what the user sees, so it can
 *   never show a message the core no longer holds.
 * - `pendingFollowupsByThread` is the composer's: the full user message behind
 *   every follow-up this client queued. The web channel never writes user
 *   messages to the transcript, so `ChatRuntimeProvider` appends these when the
 *   turn ends, after that turn's reply. A queue item only carries an 80-char
 *   preview, which is why the message itself has to be kept here.
 *
 * The two are linked by preview text only. The core mints the item id and the
 * `channel_web_chat` ack does not return it, so removing an item drops the
 * pending follow-up whose preview matches, keeping a cancelled message out of
 * the transcript.
 */
import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import type { QueueItemPayload } from '../services/chatService';
import type { ThreadMessage } from '../types/thread';
import { clearAllChatRuntime, clearRuntimeForThread, endInferenceTurn } from './chatRuntimeSlice';
import { resetUserScopedState } from './resetActions';

/** A message waiting in a running turn's core queue. */
export interface RunQueueItem {
  /** The core's queue item id (`QueueItemPayload.id`). */
  id: string;
  /** `steer` / `followup` / `collect` when the core names it. */
  lane: string | null;
  /** The core's clipped preview of the message text. */
  textPreview: string;
}

/** A follow-up this client queued, held until it can be persisted. */
export interface PendingFollowup {
  /** The user message exactly as an interactive send would store it. */
  message: ThreadMessage;
  /** `clipQueuePreview` of the text sent to the core; matches the item's preview. */
  preview: string;
}

export interface QueueState {
  itemsByThread: Record<string, RunQueueItem[]>;
  pendingFollowupsByThread: Record<string, PendingFollowup[]>;
}

const initialState: QueueState = { itemsByThread: {}, pendingFollowupsByThread: {} };

/** Mirrors the core's `queued_turn::text_preview`: 80 code points, then `…`. */
const QUEUE_PREVIEW_CHARS = 80;

export function clipQueuePreview(text: string): string {
  const chars = Array.from(text);
  if (chars.length <= QUEUE_PREVIEW_CHARS) return text;
  return `${chars.slice(0, QUEUE_PREVIEW_CHARS).join('')}…`;
}

function dropItem(state: QueueState, threadId: string, itemId: string): RunQueueItem | null {
  const bucket = state.itemsByThread[threadId];
  const index = bucket?.findIndex(item => item.id === itemId) ?? -1;
  if (!bucket || index === -1) return null;
  const [removed] = bucket.splice(index, 1);
  if (bucket.length === 0) delete state.itemsByThread[threadId];
  return removed;
}

function clearThread(state: QueueState, threadId: string) {
  delete state.itemsByThread[threadId];
  delete state.pendingFollowupsByThread[threadId];
}

const queueSlice = createSlice({
  name: 'queue',
  initialState,
  reducers: {
    queueItemQueued: (state, action: PayloadAction<{ threadId: string; item: QueueItemPayload }>) => {
      const { threadId, item } = action.payload;
      const bucket = state.itemsByThread[threadId] ?? [];
      if (bucket.some(existing => existing.id === item.id)) return;
      bucket.push({ id: item.id, lane: item.lane ?? null, textPreview: item.text_preview ?? '' });
      state.itemsByThread[threadId] = bucket;
    },
    /** The core handed the item to a turn; its message persists on turn end. */
    queueItemDelivered: (state, action: PayloadAction<{ threadId: string; itemId: string }>) => {
      dropItem(state, action.payload.threadId, action.payload.itemId);
    },
    /** The item was taken out of the queue and will never be sent. */
    queueItemRemoved: (state, action: PayloadAction<{ threadId: string; itemId: string }>) => {
      const { threadId, itemId } = action.payload;
      const removed = dropItem(state, threadId, itemId);
      const pending = state.pendingFollowupsByThread[threadId];
      if (!removed || !pending) return;
      const index = pending.findIndex(entry => entry.preview === removed.textPreview);
      if (index === -1) return;
      pending.splice(index, 1);
      if (pending.length === 0) delete state.pendingFollowupsByThread[threadId];
    },
    /** Record a follow-up the core accepted; `text` is what was sent to it. */
    pendingFollowupAdded: (
      state,
      action: PayloadAction<{ threadId: string; message: ThreadMessage; text: string }>
    ) => {
      const { threadId, message, text } = action.payload;
      const bucket = state.pendingFollowupsByThread[threadId] ?? [];
      bucket.push({ message, preview: clipQueuePreview(text) });
      state.pendingFollowupsByThread[threadId] = bucket;
    },
  },
  extraReducers: builder => {
    // The turn ended, so the core is dispatching whatever it still queued, and
    // `ChatRuntimeProvider` has already persisted the pending follow-ups.
    builder.addCase(endInferenceTurn, (state, action) => clearThread(state, action.payload.threadId));
    builder.addCase(clearRuntimeForThread, (state, action) =>
      clearThread(state, action.payload.threadId)
    );
    builder.addCase(clearAllChatRuntime, () => initialState);
    builder.addCase(resetUserScopedState, () => initialState);
  },
});

export const { queueItemQueued, queueItemDelivered, queueItemRemoved, pendingFollowupAdded } =
  queueSlice.actions;

export default queueSlice.reducer;
