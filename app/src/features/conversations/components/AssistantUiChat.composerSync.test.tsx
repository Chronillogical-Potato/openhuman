/**
 * Composer ↔ host draft sync under fast typing.
 *
 * `ComposerTextBridge` mirrored every keystroke into the host with a sync
 * `setState` inside an effect. A burst of keystrokes — key-repeat, fast
 * typing, an automated driver — chained those synchronous updates past React's
 * nested-update limit ("Maximum update depth exceeded") and the chat surface
 * fell to the error boundary. Reproduced in Brave against the dev build by
 * typing a one-line prompt at driver speed.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import mascotReducer from '../../../store/mascotSlice';
import threadReducer from '../../../store/threadSlice';
import { AssistantUiChat } from './AssistantUiChat';

const THREAD_ID = 't-composer-sync';

function buildStore() {
  return configureStore({
    reducer: combineReducers({
      thread: threadReducer,
      chatRuntime: chatRuntimeReducer,
      mascot: mascotReducer,
    }),
    preloadedState: {
      thread: {
        threads: [],
        selectedThreadId: THREAD_ID,
        activeThreadIds: {},
        welcomeThreadId: null,
        messagesByThreadId: { [THREAD_ID]: [] },
        messages: [],
        isLoadingThreads: false,
        isLoadingMessages: false,
        messagesError: null,
      },
    } as never,
  });
}

let hostSet: (value: string) => void = () => {};
let hostValue = '';

/** A host that owns the draft, like `Conversations` does. */
function Host() {
  const [value, setValue] = useState('');
  hostSet = setValue;
  hostValue = value;
  return (
    <AssistantUiChat
      model={null}
      onModelChange={vi.fn()}
      inputValue={value}
      onInputValueChange={setValue}
      attachments={[]}
      onAttachFiles={vi.fn()}
      onRemoveAttachment={vi.fn()}
      maxAttachments={5}
      attachmentsEnabled={false}
      attachmentInteractionBlocked={false}
      onAttachmentOnlySend={vi.fn()}
    />
  );
}

function composer(): HTMLTextAreaElement {
  return screen.getByRole('textbox') as HTMLTextAreaElement;
}

describe('ComposerTextBridge', () => {
  it('survives a burst of keystrokes and the host ends on what was typed', async () => {
    render(
      <Provider store={buildStore()}>
        <Host />
      </Provider>
    );
    const text = 'Check the config, then search for the setting, then explain it.';
    // One discrete input event per character, back to back, as key-repeat or a
    // fast typist produces them.
    for (let n = 1; n <= text.length; n += 1) {
      fireEvent.change(composer(), { target: { value: text.slice(0, n) } });
    }

    expect(screen.queryByText(/Something went wrong/)).toBeNull();
    expect(composer().value).toBe(text);
    await waitFor(() => expect(hostValue).toBe(text));
    // The lagging echoes never overwrote what was typed after them.
    expect(composer().value).toBe(text);
  });

  it('still lets a host-side write (clear, restore, dictation) win', async () => {
    render(
      <Provider store={buildStore()}>
        <Host />
      </Provider>
    );
    fireEvent.change(composer(), { target: { value: 'draft' } });
    await waitFor(() => expect(hostValue).toBe('draft'));

    act(() => hostSet('restored from history'));
    await waitFor(() => expect(composer().value).toBe('restored from history'));

    act(() => hostSet(''));
    await waitFor(() => expect(composer().value).toBe(''));
  });
});
