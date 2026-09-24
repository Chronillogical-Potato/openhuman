/**
 * The runtime reads must degrade, not throw, when no runtime is mounted — and
 * must report the ADAPTER's real capabilities when one is.
 *
 * The capability half is the important one. `useOpenHumanExternalStore` now
 * implements `onEdit` (via `threads.edit_message`) and `setMessages` (a no-op
 * stub that exists only to un-gate `BranchPicker` — the core has no
 * per-branch message model yet, so `onReload`/`onEdit` both truncate the
 * thread's single lineage rather than forking one), so assistant-ui reports
 * `edit` and `switchToBranch` as true. `thread.tsx`'s `EditComposer` and
 * `BranchPickerPrimitive` are gated on this hook, and this test is what would
 * fail the day the adapter stops honouring either affordance.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it } from 'vitest';

import { AssistantUiRuntimeProvider } from '../../../../providers/AssistantUiRuntimeProvider';
import chatRuntimeReducer from '../../../../store/chatRuntimeSlice';
import threadReducer from '../../../../store/threadSlice';
import { useAuiEditCapabilities } from './auiThreadState';

function withRuntime(threadId: string | null) {
  const store = configureStore({
    reducer: combineReducers({ thread: threadReducer, chatRuntime: chatRuntimeReducer }),
  });
  return ({ children }: { children: ReactNode }) => (
    <Provider store={store}>
      <AssistantUiRuntimeProvider threadId={threadId}>{children}</AssistantUiRuntimeProvider>
    </Provider>
  );
}

describe('auiThreadState', () => {
  it('reports no edit or branch capability with no runtime mounted', () => {
    const { result } = renderHook(() => useAuiEditCapabilities());
    expect(result.current).toEqual({ canEdit: false, canSwitchToBranch: false });
  });

  it('reports the external-store adapter as supporting both edit and branching', () => {
    const { result } = renderHook(() => useAuiEditCapabilities(), {
      wrapper: withRuntime('t-caps'),
    });
    expect(result.current).toEqual({ canEdit: true, canSwitchToBranch: true });
  });
});
