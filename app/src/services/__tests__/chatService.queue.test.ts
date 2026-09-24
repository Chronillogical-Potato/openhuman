import { beforeEach, describe, expect, it, vi } from 'vitest';

import { chatRemoveQueueItem, subscribeQueueEvents } from '../chatService';
import { socketService } from '../socketService';

const mockCallCoreRpc = vi.fn();

vi.mock('../socketService', () => ({
  socketService: { getSocket: vi.fn(), on: vi.fn(), off: vi.fn() },
}));
vi.mock('../coreRpcClient', () => ({
  callCoreRpc: (...args: unknown[]) => mockCallCoreRpc(...args),
}));

type Handler = (...args: unknown[]) => void;

function bindMockSocket() {
  const handlers = new Map<string, Handler[]>();
  vi.mocked(socketService.getSocket).mockReturnValue({ id: 'socket-1' } as never);
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

beforeEach(() => {
  vi.clearAllMocks();
  mockCallCoreRpc.mockReset();
});

describe('chatService.subscribeQueueEvents', () => {
  it('routes queued / delivered / removed events to their listeners', () => {
    const emit = bindMockSocket();
    const onQueued = vi.fn();
    const onDelivered = vi.fn();
    const onRemoved = vi.fn();
    subscribeQueueEvents({ onQueued, onDelivered, onRemoved });

    const item = { id: 'q1', text_preview: 'hello' };
    emit('queue_item_queued', { thread_id: 't1', client_id: '', queue_item: item });
    emit('queue_item_delivered', { thread_id: 't1', client_id: '', queue_item: item });
    emit('queue_item_removed', { thread_id: 't1', client_id: '', queue_item: item });

    expect(onQueued).toHaveBeenCalledWith({ thread_id: 't1', client_id: '', queue_item: item });
    expect(onDelivered).toHaveBeenCalledWith(expect.objectContaining({ queue_item: item }));
    expect(onRemoved).toHaveBeenCalledWith(expect.objectContaining({ queue_item: item }));
  });

  it('drops an event that carries no queue item', () => {
    const emit = bindMockSocket();
    const onQueued = vi.fn();
    subscribeQueueEvents({ onQueued });

    emit('queue_item_queued', { thread_id: 't1' });

    expect(onQueued).not.toHaveBeenCalled();
  });

  it('unsubscribes every handler it registered', () => {
    const emit = bindMockSocket();
    const onQueued = vi.fn();
    const unsubscribe = subscribeQueueEvents({ onQueued, onDelivered: vi.fn() });

    unsubscribe();
    emit('queue_item_queued', { thread_id: 't1', queue_item: { id: 'q1' } });

    expect(onQueued).not.toHaveBeenCalled();
    expect(socketService.off).toHaveBeenCalledTimes(2);
  });
});

describe('chatService.chatRemoveQueueItem', () => {
  it('asks the core to drop one item and reports success', async () => {
    bindMockSocket();
    mockCallCoreRpc.mockResolvedValueOnce({ removed: true });

    expect(await chatRemoveQueueItem('t1', 'q1')).toBe(true);
    expect(mockCallCoreRpc).toHaveBeenCalledWith({
      method: 'openhuman.channel_web_queue_remove',
      params: { client_id: 'socket-1', thread_id: 't1', item_id: 'q1' },
    });
  });

  it('reports failure when the core did not remove it', async () => {
    bindMockSocket();
    mockCallCoreRpc.mockResolvedValueOnce({ removed: false });

    expect(await chatRemoveQueueItem('t1', 'q1')).toBe(false);
  });

  it('reports failure when the RPC rejects (e.g. an older core without the method)', async () => {
    bindMockSocket();
    mockCallCoreRpc.mockRejectedValueOnce(new Error('unknown method'));

    expect(await chatRemoveQueueItem('t1', 'q1')).toBe(false);
  });

  it('does not call the core without a socket id', async () => {
    vi.mocked(socketService.getSocket).mockReturnValue(null as never);

    expect(await chatRemoveQueueItem('t1', 'q1')).toBe(false);
    expect(mockCallCoreRpc).not.toHaveBeenCalled();
  });
});
