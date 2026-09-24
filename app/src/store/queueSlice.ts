import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import type { ThreadMessage } from '../types/thread';

export const clipQueuePreview = (text: string): string => text;

const slice = createSlice({
  name: 'queue',
  initialState: { itemsByThread: {} as Record<string, unknown[]>, pendingFollowupsByThread: {} as Record<string, unknown[]> },
  reducers: {
    queueItemQueued: (_s, _a: PayloadAction<{ threadId: string; item: { id: string; lane?: string | null; text_preview?: string | null } }>) => {},
    queueItemDelivered: (_s, _a: PayloadAction<{ threadId: string; itemId: string }>) => {},
    queueItemRemoved: (_s, _a: PayloadAction<{ threadId: string; itemId: string }>) => {},
    pendingFollowupAdded: (_s, _a: PayloadAction<{ threadId: string; message: ThreadMessage; text: string }>) => {},
  },
});
export const { queueItemQueued, queueItemDelivered, queueItemRemoved, pendingFollowupAdded } = slice.actions;
export default slice.reducer;
