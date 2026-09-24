import { beforeEach, describe, expect, it, vi } from 'vitest';

import { subscribeSuggestionEvents } from '../chatService';
import { socketService } from '../socketService';

vi.mock('../socketService', () => ({
  socketService: { getSocket: vi.fn(), on: vi.fn(), off: vi.fn() },
}));
vi.mock('../coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

type Handler = (...args: unknown[]) => void;

function bindMockSocket() {
  const handlers = new Map<string, Handler[]>();
  vi.mocked(socketService.on).mockImplementation((event, cb) => {
    handlers.set(event, [...(handlers.get(event) ?? []), cb as Handler]);
  });
  vi.mocked(socketService.off).mockImplementation((event, cb) => {
    handlers.set(
      event,
      (handlers.get(event) ?? []).filter(handler => handler !== cb)
    );
  });
  return (event: string, payload: unknown) => {
    for (const handler of handlers.get(event) ?? []) handler(payload);
  };
}

beforeEach(() => vi.clearAllMocks());

describe('chatService.subscribeSuggestionEvents', () => {
  it('routes a chat_suggestions event to the listener', () => {
    const emit = bindMockSocket();
    const onSuggestions = vi.fn();
    subscribeSuggestionEvents({ onSuggestions });

    const event = {
      thread_id: 't1',
      client_id: 'c1',
      request_id: '',
      turn_request_id: 'r1',
      suggestions: [{ prompt: 'What about tomorrow?', label: 'Tomorrow' }],
    };
    emit('chat_suggestions', event);

    expect(onSuggestions).toHaveBeenCalledWith(event);
  });

  it('drops an event with no thread id or no suggestion list', () => {
    const emit = bindMockSocket();
    const onSuggestions = vi.fn();
    subscribeSuggestionEvents({ onSuggestions });

    emit('chat_suggestions', { thread_id: 't1' });
    emit('chat_suggestions', { suggestions: [{ prompt: 'x' }] });
    emit('chat_suggestions', null);

    expect(onSuggestions).not.toHaveBeenCalled();
  });

  it('unsubscribes the handler it registered', () => {
    const emit = bindMockSocket();
    const onSuggestions = vi.fn();
    const unsubscribe = subscribeSuggestionEvents({ onSuggestions });

    unsubscribe();
    emit('chat_suggestions', { thread_id: 't1', suggestions: [{ prompt: 'x' }] });

    expect(onSuggestions).not.toHaveBeenCalled();
  });
});
