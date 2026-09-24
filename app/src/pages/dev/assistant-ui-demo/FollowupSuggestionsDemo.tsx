/**
 * Dev-only fixture for the vendored follow-up-suggestions element
 * (`/dev/tools`). Runs the fixture `chat_suggestions` event through the app's
 * own reducer and chip mapping, then renders `ThreadFollowupSuggestions` on a
 * tiny in-memory runtime holding one settled turn. Clicking a chip is a no-op
 * here; nothing reaches the core.
 */
import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import debugFactory from 'debug';
import { useMemo } from 'react';

import { ThreadFollowupSuggestions } from '../../../components/assistant-ui/follow-up-suggestions';
import followupSuggestionsReducer, {
  followupSuggestionsReceived,
  toThreadSuggestions,
} from '../../../store/followupSuggestionsSlice';
import { MOCK_CHAT_SUGGESTIONS_EVENT, MOCK_SUGGESTIONS_TURN } from './assistantUiMock/mockScript';

const debug = debugFactory('openhuman:assistant-ui-demo');

const MESSAGES: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: MOCK_SUGGESTIONS_TURN.user }] },
  { role: 'assistant', content: [{ type: 'text', text: MOCK_SUGGESTIONS_TURN.assistant }] },
];

export function FollowupSuggestionsDemo() {
  const suggestions = useMemo(() => {
    const event = MOCK_CHAT_SUGGESTIONS_EVENT;
    const state = followupSuggestionsReducer(
      undefined,
      followupSuggestionsReceived({
        threadId: event.thread_id,
        requestId: event.turn_request_id ?? null,
        suggestions: event.suggestions,
      })
    );
    return toThreadSuggestions(state.byThread[event.thread_id]?.suggestions ?? []);
  }, []);

  const runtime = useExternalStoreRuntime({
    messages: MESSAGES,
    isRunning: false,
    suggestions,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {
      debug('[assistant-ui-demo] follow-up chip clicked (mock, discarded)');
    },
  });

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <ThreadFollowupSuggestions />
    </AssistantRuntimeProvider>
  );
}

export default FollowupSuggestionsDemo;
