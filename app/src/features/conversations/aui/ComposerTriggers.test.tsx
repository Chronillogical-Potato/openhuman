import {
  AssistantRuntimeProvider,
  ComposerPrimitive,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../../services/coreRpcClient';
import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import runModeReducer from '../../../store/runModeSlice';
import { ComposerTriggers } from './ComposerTriggers';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

function Harness({ children }: { children: ReactNode }) {
  const messages: ThreadMessageLike[] = [];
  const runtime = useExternalStoreRuntime({
    messages,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}

function renderComposer() {
  const store = configureStore({
    reducer: combineReducers({ chatRuntime: chatRuntimeReducer, runMode: runModeReducer }),
  });
  render(
    <Provider store={store}>
      <Harness>
        <ComposerPrimitive.Unstable_TriggerPopoverRoot>
          <ComposerPrimitive.Root>
            <ComposerPrimitive.Input aria-label="Message input" />
            <ComposerTriggers />
          </ComposerPrimitive.Root>
        </ComposerPrimitive.Unstable_TriggerPopoverRoot>
      </Harness>
    </Provider>
  );
  return screen.getByRole('textbox', { name: 'Message input' });
}

async function type(input: HTMLElement, value: string) {
  await act(async () => {
    fireEvent.change(input, { target: { value } });
  });
}

describe('ComposerTriggers', () => {
  beforeEach(() => {
    vi.mocked(callCoreRpc).mockReset();
    vi.mocked(callCoreRpc).mockResolvedValue({});
  });

  it('opens the slash popover with the builtin commands on `/`', async () => {
    const input = renderComposer();
    await type(input, '/pl');

    const popover = await screen.findByTestId('composer-slash-popover');
    expect(popover).toHaveTextContent('/plan');
    expect(popover).toHaveTextContent('Plan first: review the steps before anything runs');
  });

  it('opens the mention popover with Memory and Files on `@`', async () => {
    const input = renderComposer();
    await type(input, 'look at @');

    const popover = await screen.findByTestId('composer-mention-popover');
    await waitFor(() => expect(popover).toHaveTextContent('Memory'));
    expect(popover).toHaveTextContent('Files');
  });
});
