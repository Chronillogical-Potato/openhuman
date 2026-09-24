import { describe, expect, it } from 'vitest';

import {
  beginInferenceTurn,
  clearAllChatRuntime,
  clearRuntimeForThread,
  markInferenceTurnStreaming,
} from './chatRuntimeSlice';
import followupSuggestionsReducer, {
  followupSuggestionsReceived,
  type FollowupSuggestionsState,
} from './followupSuggestionsSlice';
import { resetUserScopedState } from './resetActions';
import { truncateMessagesFrom } from './threadSlice';

const initial = followupSuggestionsReducer(undefined, { type: '@@INIT' });

function withSuggestions(...threadIds: string[]): FollowupSuggestionsState {
  return threadIds.reduce(
    (state, threadId) =>
      followupSuggestionsReducer(
        state,
        followupSuggestionsReceived({
          threadId,
          requestId: `r-${threadId}`,
          suggestions: [{ prompt: `next for ${threadId}`, label: 'Next' }],
        })
      ),
    initial
  );
}

describe('followupSuggestionsSlice', () => {
  it('stores the latest suggestions per thread, replacing the previous set', () => {
    let state = withSuggestions('t1');
    state = followupSuggestionsReducer(
      state,
      followupSuggestionsReceived({
        threadId: 't1',
        requestId: 'r2',
        suggestions: [{ prompt: 'newer' }],
      })
    );

    expect(state.byThread.t1).toEqual({
      requestId: 'r2',
      suggestions: [{ prompt: 'newer', label: null }],
    });
  });

  it('drops blank prompts, trims labels, and stores nothing for an empty set', () => {
    let state = followupSuggestionsReducer(
      initial,
      followupSuggestionsReceived({
        threadId: 't1',
        requestId: null,
        suggestions: [
          { prompt: '  ' },
          { prompt: ' Keep me ', label: '  ' },
          { prompt: 'x', label: ' Short ' },
        ],
      })
    );
    expect(state.byThread.t1.suggestions).toEqual([
      { prompt: 'Keep me', label: null },
      { prompt: 'x', label: 'Short' },
    ]);

    state = followupSuggestionsReducer(
      state,
      followupSuggestionsReceived({
        threadId: 't1',
        requestId: null,
        suggestions: [{ prompt: '' }],
      })
    );
    expect(state.byThread.t1).toBeUndefined();
  });

  it('clears a thread when a new send starts on it, leaving other threads alone', () => {
    const state = followupSuggestionsReducer(
      withSuggestions('t1', 't2'),
      beginInferenceTurn({ threadId: 't1' })
    );

    expect(state.byThread.t1).toBeUndefined();
    expect(state.byThread.t2).toBeDefined();
  });

  it('clears a thread when the core starts a turn on it (edit, regenerate, queued follow-up)', () => {
    const state = followupSuggestionsReducer(
      withSuggestions('t1'),
      markInferenceTurnStreaming({ threadId: 't1' })
    );

    expect(state.byThread.t1).toBeUndefined();
  });

  it('clears a thread whose tail was truncated (edit, reload, discard)', () => {
    const state = followupSuggestionsReducer(
      withSuggestions('t1'),
      truncateMessagesFrom({ threadId: 't1', messageId: 'm1', inclusive: true })
    );

    expect(state.byThread.t1).toBeUndefined();
  });

  it('clears on a runtime reset for the thread, and everything on a global reset', () => {
    expect(
      followupSuggestionsReducer(withSuggestions('t1'), clearRuntimeForThread({ threadId: 't1' }))
        .byThread.t1
    ).toBeUndefined();
    expect(followupSuggestionsReducer(withSuggestions('t1', 't2'), clearAllChatRuntime())).toEqual(
      initial
    );
    expect(followupSuggestionsReducer(withSuggestions('t1'), resetUserScopedState())).toEqual(
      initial
    );
  });
});
