/**
 * The agent-process-source command is offered on every chat surface.
 *
 * `showProcessSource` only drives `TranscriptOverlays`, which mounts inside the
 * assistant-ui panel. That panel used to be one half of an either/or — voice
 * (`mic-cloud`) mode mounted a separate legacy transcript instead, where the
 * state the command set had no host, so the command had to be disabled there.
 * Voice mode now renders the same assistant-ui panel with only the composer
 * swapped, so the overlays (and the command) are live in both modes.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, cleanup, render } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { SidebarSlotOutlet, SidebarSlotProvider } from '../../components/layout/shell/SidebarSlot';
import { registry } from '../../lib/commands/registry';
import chatRuntimeReducer from '../../store/chatRuntimeSlice';
import layoutReducer from '../../store/layoutSlice';
import runModeReducer from '../../store/runModeSlice';
import socketReducer from '../../store/socketSlice';
import themeReducer from '../../store/themeSlice';
import threadGoalReducer from '../../store/threadGoalSlice';
import threadReducer from '../../store/threadSlice';
import threadTodosReducer from '../../store/threadTodosSlice';
import type { Thread } from '../../types/thread';

const { mockGetThreads, mockGetThreadMessages, mockUseUsageState } = vi.hoisted(() => ({
  mockGetThreads: vi.fn().mockResolvedValue({ threads: [], count: 0 }),
  mockGetThreadMessages: vi.fn().mockResolvedValue({ messages: [], count: 0 }),
  mockUseUsageState: vi.fn(() => ({
    teamUsage: null,
    currentPlan: null,
    currentTier: 'FREE' as const,
    isFreeTier: true,
    usagePct: 0,
    isNearLimit: false,
    isAtLimit: false,
    isBudgetExhausted: false,
    shouldShowBudgetCompletedMessage: false,
    isLoading: false,
    refresh: vi.fn(),
  })),
}));

vi.mock('../../services/chatService', () => ({
  chatCancel: vi.fn().mockResolvedValue({ accepted: true, turnCancelled: true }),
  chatClearQueue: vi.fn().mockResolvedValue(0),
  chatSend: vi.fn().mockResolvedValue(undefined),
  subscribeChatEvents: vi.fn(() => () => {}),
  useRustChat: vi.fn(() => true),
}));

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    createNewThread: vi.fn().mockResolvedValue({ id: 'new-thread', labels: [] }),
    getThreads: mockGetThreads,
    getThreadMessages: mockGetThreadMessages,
    getTurnState: vi.fn().mockResolvedValue(null),
    getTurnStateHistory: vi.fn().mockResolvedValue([]),
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({
        threadId: 'none',
        items: [],
        total: 0,
        hasMore: false,
        hasTranscript: false,
      }),
    appendMessage: vi.fn(async (_threadId: string, message: unknown) => message),
    deleteThread: vi.fn().mockResolvedValue({ deleted: true }),
    generateTitleIfNeeded: vi.fn().mockResolvedValue({}),
    updateMessage: vi.fn().mockResolvedValue({}),
    purge: vi.fn().mockResolvedValue({}),
    updateLabels: vi.fn().mockResolvedValue({}),
    updateTitle: vi.fn().mockResolvedValue({}),
    persistReaction: vi.fn().mockResolvedValue({}),
    listRuns: vi.fn().mockResolvedValue([]),
    listRunEvents: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock('../../hooks/useUsageState', () => ({ useUsageState: mockUseUsageState }));

vi.mock('../../lib/coreState/store', () => ({
  getCoreStateSnapshot: vi.fn(() => ({
    isBootstrapping: false,
    isReady: true,
    snapshot: {
      auth: { isAuthenticated: false, userId: null, user: null, profileId: null },
      sessionToken: null,
      currentUser: null,
      onboardingCompleted: true,
      chatOnboardingCompleted: true,
      analyticsEnabled: false,
      localState: {},
      runtime: {},
    },
  })),
  isWelcomeLocked: vi.fn(() => false),
  setCoreStateSnapshot: vi.fn(),
}));

const THREAD_ID = 'process-source-thread';

const thread: Thread = {
  id: THREAD_ID,
  title: 'Process source thread',
  chatId: null,
  isActive: false,
  messageCount: 0,
  lastMessageAt: '2026-01-01T00:00:00.000Z',
  createdAt: '2026-01-01T00:00:00.000Z',
  labels: ['general'],
};

const ACTION_ID = 'chat.agentProcessSource';

function buildStore(preload: Record<string, unknown>) {
  return configureStore({
    reducer: combineReducers({
      thread: threadReducer,
      layout: layoutReducer,
      socket: socketReducer,
      chatRuntime: chatRuntimeReducer,
      theme: themeReducer,
    }),
    preloadedState: preload as never,
  });
}

async function renderChat(composer?: 'text' | 'mic-cloud', withProcessData = false) {
  mockGetThreads.mockResolvedValue({ threads: [thread], count: 1 });
  const store = buildStore({
    thread: {
      threads: [thread],
      selectedThreadId: THREAD_ID,
      activeThreadIds: {},
      welcomeThreadId: null,
      messagesByThreadId: { [THREAD_ID]: [] },
      messages: [],
      isLoadingThreads: false,
      isLoadingMessages: false,
      messagesError: null,
    },
    socket: { byUser: { __pending__: { status: 'connected', socketId: 'socket-1' } } },
    ...(withProcessData
      ? {
          chatRuntime: {
            ...chatRuntimeReducer(undefined, { type: '@@init' }),
            toolTimelineByThread: {
              [THREAD_ID]: [{ id: 'c1', name: 'web_fetch', round: 1, seq: 0, status: 'success' }],
            },
          },
        }
      : {}),
  });
  const { default: Conversations } = await import('./Conversations');

  await act(async () => {
    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/chat']}>
          <SidebarSlotProvider>
            <SidebarSlotOutlet />
            <Conversations composer={composer} />
          </SidebarSlotProvider>
        </MemoryRouter>
      </Provider>
    );
  });
}

// The predicate's other half (`selectedThreadId !== null`) is deliberately not
// asserted here: on `/chat` it is unreachable as a steady state. The boot effect
// reuses an empty thread or calls `handleCreateNewThread`, so the page always
// ends up with a selection and a test for it would only be pinning the mock.
describe('the agent-process-source command follows the panel that hosts it', () => {
  afterEach(() => {
    cleanup();
    registry.reset();
  });

  it('is disabled when the assistant-ui surface has no process data to show', async () => {
    await renderChat('text');

    const action = registry.getAction(ACTION_ID);
    expect(action, 'the command must be registered on the text composer').toBeDefined();
    expect(action?.enabled?.()).toBe(false);
    // The palette runs it through `runAction`, which re-checks `enabled`.
    expect(registry.runAction(ACTION_ID)).toBe(false);
  });

  it('is enabled in mic-cloud voice mode too, which renders the same assistant-ui panel', async () => {
    // With something to show: the command is gated on process data (above),
    // and voice mode must not add a gate of its own.
    await renderChat('mic-cloud', true);

    // Voice mode swaps only the composer: the transcript is the assistant-ui
    // viewport and the text composer is replaced by the voice composer.
    expect(document.querySelector('[data-slot="aui_thread-viewport"]')).not.toBeNull();
    expect(document.querySelector('[data-testid="voice-composer"]')).not.toBeNull();
    expect(document.querySelector('[data-slot="aui_composer-shell"]')).toBeNull();

    const action = registry.getAction(ACTION_ID);
    expect(action, 'the command is registered in voice mode').toBeDefined();
    expect(action?.enabled?.()).toBe(true);
    expect(registry.runAction(ACTION_ID)).toBe(true);
  });
});
