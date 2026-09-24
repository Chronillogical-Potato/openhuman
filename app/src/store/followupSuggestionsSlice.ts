/**
 * Follow-up suggestions per thread: the chips offered under a settled turn.
 *
 * Filled by the core's `chat_suggestions` socket event (`web_chat/suggestions.rs`),
 * which arrives after `chat_done` for a normal single-user turn. Best effort:
 * a thread with no entry just shows no follow-up chips.
 *
 * Holds only the set for the thread's latest turn. It is cleared whenever that
 * turn stops being the latest one: a send starts (`beginInferenceTurn`), the
 * core starts a turn this client did not send, such as an edit, a regenerate
 * or a queued follow-up (`markInferenceTurnStreaming`, driven by
 * `inference_start`), or the transcript tail is cut (`truncateMessagesFrom`).
 *
 * Only `useOpenHumanExternalStore` reads this, and only after a settled turn.
 * See `useThreadSuggestions` there for why the welcome and follow-up chips
 * must never show together.
 */
import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import {
  beginInferenceTurn,
  clearAllChatRuntime,
  clearRuntimeForThread,
  markInferenceTurnStreaming,
} from './chatRuntimeSlice';
import { resetUserScopedState } from './resetActions';
import { truncateMessagesFrom } from './threadSlice';

/** One follow-up chip: `label` is its short button text, `prompt` what it sends. */
export interface FollowupSuggestion {
  prompt: string;
  label: string | null;
}

export interface ThreadFollowupSuggestions {
  /** The turn these follow (`turn_request_id`), when the core names it. */
  requestId: string | null;
  suggestions: FollowupSuggestion[];
}

export interface FollowupSuggestionsState {
  byThread: Record<string, ThreadFollowupSuggestions>;
}

const initialState: FollowupSuggestionsState = { byThread: {} };

function normalize(
  raw: ReadonlyArray<{ prompt?: string | null; label?: string | null }>
): FollowupSuggestion[] {
  const out: FollowupSuggestion[] = [];
  for (const entry of raw) {
    const prompt = typeof entry.prompt === 'string' ? entry.prompt.trim() : '';
    if (prompt.length === 0) continue;
    const label = typeof entry.label === 'string' ? entry.label.trim() : '';
    out.push({ prompt, label: label.length > 0 ? label : null });
  }
  return out;
}

function clearThread(state: FollowupSuggestionsState, threadId: string) {
  delete state.byThread[threadId];
}

const followupSuggestionsSlice = createSlice({
  name: 'followupSuggestions',
  initialState,
  reducers: {
    followupSuggestionsReceived: (
      state,
      action: PayloadAction<{
        threadId: string;
        requestId: string | null;
        suggestions: ReadonlyArray<{ prompt?: string | null; label?: string | null }>;
      }>
    ) => {
      const { threadId, requestId } = action.payload;
      const suggestions = normalize(action.payload.suggestions);
      if (suggestions.length === 0) {
        clearThread(state, threadId);
        return;
      }
      state.byThread[threadId] = { requestId, suggestions };
    },
  },
  extraReducers: builder => {
    builder.addCase(beginInferenceTurn, (state, action) =>
      clearThread(state, action.payload.threadId)
    );
    builder.addCase(markInferenceTurnStreaming, (state, action) =>
      clearThread(state, action.payload.threadId)
    );
    builder.addCase(truncateMessagesFrom, (state, action) =>
      clearThread(state, action.payload.threadId)
    );
    builder.addCase(clearRuntimeForThread, (state, action) =>
      clearThread(state, action.payload.threadId)
    );
    builder.addCase(clearAllChatRuntime, () => initialState);
    builder.addCase(resetUserScopedState, () => initialState);
  },
});

export const { followupSuggestionsReceived } = followupSuggestionsSlice.actions;

export default followupSuggestionsSlice.reducer;
