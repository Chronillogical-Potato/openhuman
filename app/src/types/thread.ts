export interface Thread {
  id: string;
  title: string;
  chatId: number | null;
  isActive: boolean;
  messageCount: number;
  lastMessageAt: string;
  createdAt: string;
  parentThreadId?: string;
  labels: string[];
  personalityId?: string | null;
  /** Working folder the thread's agent acts in; absent means the global default. */
  actionDir?: string | null;
}

export interface ThreadMessage {
  id: string;
  content: string;
  type: string;
  extraMetadata: Record<string, unknown>;
  sender: 'user' | 'agent';
  createdAt: string;
}

export interface ThreadsListData {
  threads: Thread[];
  count: number;
}

export interface ThreadMessagesData {
  messages: ThreadMessage[];
  count: number;
}

export interface ThreadDeleteData {
  deleted: boolean;
}

export interface PurgeResultData {
  messagesDeleted: number;
  agentThreadsDeleted: number;
  agentMessagesDeleted: number;
}

/** One message matched by `openhuman.threads_search` (global search). */
export interface ThreadSearchHit {
  threadId: string;
  messageId: string;
  role: string;
  /** Message text around the match, `…` where it was cut. */
  snippet: string;
  createdAt: string;
}
