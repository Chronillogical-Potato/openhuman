/**
 * Mirror the core's `chat_suggestions` socket events into
 * `followupSuggestionsSlice`, which the external-store adapter reads to offer
 * follow-up chips under a settled turn. Mounted once, by `ChatRuntimeProvider`,
 * while the socket is connected: events are keyed by thread, so a turn that
 * settles off screen still has its chips when the user returns to it.
 */
import { useEffect } from 'react';

import { subscribeSuggestionEvents } from '../../../services/chatService';
import { followupSuggestionsReceived } from '../../../store/followupSuggestionsSlice';
import { useAppDispatch } from '../../../store/hooks';

export function useFollowupSuggestionEvents(enabled: boolean): void {
  const dispatch = useAppDispatch();

  useEffect(() => {
    if (!enabled) return;
    return subscribeSuggestionEvents({
      onSuggestions: e =>
        dispatch(
          followupSuggestionsReceived({
            threadId: e.thread_id,
            requestId: e.turn_request_id || e.request_id || null,
            suggestions: e.suggestions,
          })
        ),
    });
  }, [dispatch, enabled]);
}
