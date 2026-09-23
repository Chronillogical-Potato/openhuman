/**
 * The runtime must actually offer speech, or the Read aloud button never
 * renders in the real app.
 *
 * `thread.readAloud.test.tsx` proves the buttons appear when a runtime CAN
 * speak, using its own adapter. This is the other half: that OpenHuman's own
 * adapter supplies one. Without it `capabilities.speech` is false, the gate in
 * `AssistantActionBar` hides the control, and the feature is invisible on
 * `/chat` while every component-level test still passes — the stranded-surface
 * shape this whole change exists to avoid.
 */
import { configureStore } from '@reduxjs/toolkit';
import { renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import chatRuntimeReducer from '../../store/chatRuntimeSlice';
import threadReducer from '../../store/threadSlice';
import { useOpenHumanExternalStore } from '../useOpenHumanExternalStore';

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({
        threadId: 't-speech',
        items: [],
        total: 0,
        hasMore: false,
        hasTranscript: false,
      }),
  },
}));

function mount() {
  const store = configureStore({
    reducer: { thread: threadReducer, chatRuntime: chatRuntimeReducer },
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Provider store={store}>{children}</Provider>
  );
  return renderHook(() => useOpenHumanExternalStore('t-speech'), { wrapper });
}

describe('the external-store adapter', () => {
  it('supplies a speech adapter, which is what makes capabilities.speech true', () => {
    const { result } = mount();

    expect(typeof result.current.adapters?.speech?.speak).toBe('function');
  });
});
