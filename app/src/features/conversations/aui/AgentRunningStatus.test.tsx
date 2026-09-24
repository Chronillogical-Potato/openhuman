import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import { AssistantUiRuntimeProvider } from '../../../providers/AssistantUiRuntimeProvider';
import chatRuntimeReducer from '../../../store/chatRuntimeSlice';
import threadReducer from '../../../store/threadSlice';
import { AgentRunningStatus } from './AgentRunningStatus';

vi.mock('../../../services/api/threadApi', () => ({
  threadApi: {
    getDerivedTranscript: vi
      .fn()
      .mockResolvedValue({
        threadId: 't-status',
        items: [],
        total: 0,
        hasMore: false,
        hasTranscript: false,
      }),
  },
}));

const THREAD_ID = 't-status';

function buildStore() {
  return configureStore({
    reducer: combineReducers({ thread: threadReducer, chatRuntime: chatRuntimeReducer }),
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

describe('AgentRunningStatus', () => {
  it('falls back to the thinking indicator when assistant-ui has no tasks', () => {
    render(
      <Provider store={buildStore()}>
        <AssistantUiRuntimeProvider>
          <AgentRunningStatus />
        </AssistantUiRuntimeProvider>
      </Provider>
    );

    expect(screen.getByTestId('agent-running-status-thinking')).toBeInTheDocument();
    expect(screen.queryByTestId('agent-running-status-tasks')).not.toBeInTheDocument();
  });
});
