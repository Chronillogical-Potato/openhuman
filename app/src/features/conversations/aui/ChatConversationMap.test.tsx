/**
 * The conversation map: `Cmd`/`Ctrl+F` find-in-conversation and a timeline
 * outline of the thread's user turns, both scoped to the live `/chat`
 * surface (mounted through `AssistantUiChat`, the same way `ChatSources.test.tsx`
 * proves its own wiring) rather than fed a hand-built `messages` prop.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { threadApi } from '../../../services/api/threadApi';
import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import mascotReducer from '../../../store/mascotSlice';
import threadReducer from '../../../store/threadSlice';
import type { ThreadMessage } from '../../../types/thread';
import { AssistantUiChat } from '../components/AssistantUiChat';

const THREAD_ID = 't-map';

function userMessage(id: string, content: string, createdAt: string): ThreadMessage {
  return { id, content, type: 'text', extraMetadata: {}, sender: 'user', createdAt };
}

function agentMessage(id: string, content: string, createdAt: string): ThreadMessage {
  return { id, content, type: 'text', extraMetadata: {}, sender: 'agent', createdAt };
}

function buildStore(messages: ThreadMessage[]) {
  return configureStore({
    reducer: combineReducers({
      thread: threadReducer,
      chatRuntime: chatRuntimeReducer,
      mascot: mascotReducer,
    }),
    preloadedState: {
      thread: {
        threads: [
          {
            id: THREAD_ID,
            title: 'Map thread',
            chatId: null,
            isActive: false,
            messageCount: messages.length,
            lastMessageAt: '2026-01-01T00:00:00.000Z',
            createdAt: '2026-01-01T00:00:00.000Z',
            labels: [],
          },
        ],
        selectedThreadId: THREAD_ID,
        activeThreadIds: {},
        welcomeThreadId: null,
        messagesByThreadId: { [THREAD_ID]: messages },
        messages,
        isLoadingThreads: false,
        isLoadingMessages: false,
        messagesError: null,
      },
    } as never,
  });
}

function renderChat(messages: ThreadMessage[]) {
  return render(
    <Provider store={buildStore(messages)}>
      <AssistantUiChat
        model={null}
        onModelChange={vi.fn()}
        inputValue=""
        onInputValueChange={vi.fn()}
        attachments={[]}
        onAttachFiles={vi.fn()}
        onRemoveAttachment={vi.fn()}
        maxAttachments={5}
        attachmentsEnabled={false}
        attachmentInteractionBlocked={false}
        onAttachmentOnlySend={vi.fn()}
      />
    </Provider>
  );
}

beforeEach(() => {
  vi.restoreAllMocks();
  vi.spyOn(threadApi, 'getDerivedTranscript').mockResolvedValue({
    items: [],
    hasTranscript: true,
    hasMore: false,
  } as never);
});

describe('ChatConversationMap', () => {
  it('mounts around the live chat surface', async () => {
    renderChat([
      userMessage('u1', 'What is the deploy schedule?', '2026-01-01T00:00:00.000Z'),
      agentMessage('a1', 'It runs nightly at 2am UTC.', '2026-01-01T00:00:05.000Z'),
    ]);

    await waitFor(() => expect(screen.getByTestId('chat-conversation-map')).toBeTruthy());
    expect(screen.getByText('It runs nightly at 2am UTC.')).toBeTruthy();
  });

  it("opens the timeline outline and lists the thread's user turns", async () => {
    renderChat([
      userMessage('u1', 'First question', '2026-01-01T00:00:00.000Z'),
      agentMessage('a1', 'First answer', '2026-01-01T00:00:05.000Z'),
      userMessage('u2', 'Second question', '2026-01-01T00:05:00.000Z'),
    ]);

    await waitFor(() => expect(screen.getByText('First answer')).toBeTruthy());
    await userEvent.click(screen.getByTestId('chat-conversation-timeline-toggle'));

    const timeline = screen.getByTestId('chat-conversation-timeline');
    expect(timeline.textContent).toContain('First question');
    expect(timeline.textContent).toContain('Second question');
  });

  it('opens the find bar on Ctrl+F and reports a match count', async () => {
    renderChat([
      userMessage('u1', 'Where is the deploy config?', '2026-01-01T00:00:00.000Z'),
      agentMessage('a1', 'It lives in deploy/config.yaml.', '2026-01-01T00:00:05.000Z'),
    ]);

    await waitFor(() => expect(screen.getByText('It lives in deploy/config.yaml.')).toBeTruthy());

    const container = screen.getByTestId('chat-conversation-map');
    container.focus();
    await userEvent.keyboard('{Control>}f{/Control}');

    const search = await screen.findByTestId('chat-conversation-search');
    const input = search.querySelector('input') as HTMLInputElement;
    await userEvent.type(input, 'deploy');

    await waitFor(() => expect(search.textContent).toContain('1/2'));
  });
});
