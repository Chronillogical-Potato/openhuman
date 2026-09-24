import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  type SuggestionEventListeners,
  subscribeSuggestionEvents,
} from '../../../services/chatService';
import followupSuggestionsReducer from '../../../store/followupSuggestionsSlice';
import { useFollowupSuggestionEvents } from './useFollowupSuggestionEvents';

// The socket is the one external boundary here; capture what the hook registers.
vi.mock('../../../services/chatService', () => ({ subscribeSuggestionEvents: vi.fn() }));

function setup() {
  const store = configureStore({
    reducer: combineReducers({ followupSuggestions: followupSuggestionsReducer }),
  });
  let listeners: SuggestionEventListeners = {};
  const unsubscribe = vi.fn();
  vi.mocked(subscribeSuggestionEvents).mockImplementation(l => {
    listeners = l;
    return unsubscribe;
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  const hook = renderHook(({ enabled }) => useFollowupSuggestionEvents(enabled), {
    wrapper,
    initialProps: { enabled: true },
  });
  return { store, listeners: () => listeners, unsubscribe, hook };
}

describe('useFollowupSuggestionEvents', () => {
  beforeEach(() => vi.mocked(subscribeSuggestionEvents).mockReset());

  it('stores a chat_suggestions event under its thread, keyed to the turn it follows', () => {
    const { store, listeners } = setup();

    act(() =>
      listeners().onSuggestions?.({
        thread_id: 't1',
        request_id: '',
        turn_request_id: 'r1',
        suggestions: [{ prompt: 'Show me tomorrow', label: 'Tomorrow' }],
      })
    );

    expect(store.getState().followupSuggestions.byThread.t1).toEqual({
      requestId: 'r1',
      suggestions: [{ prompt: 'Show me tomorrow', label: 'Tomorrow' }],
    });
  });

  it('falls back to request_id, then null, when the turn id is missing', () => {
    const { store, listeners } = setup();

    act(() =>
      listeners().onSuggestions?.({ thread_id: 't1', request_id: 'r9', suggestions: [{ prompt: 'a' }] })
    );
    act(() => listeners().onSuggestions?.({ thread_id: 't2', suggestions: [{ prompt: 'b' }] }));

    expect(store.getState().followupSuggestions.byThread.t1.requestId).toBe('r9');
    expect(store.getState().followupSuggestions.byThread.t2.requestId).toBeNull();
  });

  it('does not subscribe while disabled and unsubscribes when disabled', () => {
    const { hook, unsubscribe } = setup();
    expect(subscribeSuggestionEvents).toHaveBeenCalledTimes(1);

    hook.rerender({ enabled: false });
    expect(unsubscribe).toHaveBeenCalledTimes(1);
    expect(subscribeSuggestionEvents).toHaveBeenCalledTimes(1);
  });
});
