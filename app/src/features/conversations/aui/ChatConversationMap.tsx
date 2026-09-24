/**
 * The conversation map for one thread: a `Cmd`/`Ctrl+F` find-in-conversation
 * bar (the vendored `conversation-search` element) and an outline popover of
 * the thread's user turns (the vendored `timeline` element), both scoped to
 * this thread's own messages — no upstream assistant-ui element covers
 * either, so both are driven entirely from `useAuiState(state =>
 * state.thread.messages)`, the same read-only projection every other adapter
 * in this app uses. Nothing here writes to Redux or the core.
 *
 * Wraps `<Thread />` rather than reaching into `thread.tsx`: the shortcut and
 * the popover are chrome around the transcript, not a slot the message tree
 * itself needs to know about.
 */
import { type AssistantState, useAuiState } from '@assistant-ui/react';
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { ConversationSearch, type SearchHit } from '../../../components/assistant-ui/elements/conversation-search';
import { Timeline, type TimelineEvent } from '../../../components/assistant-ui/elements/timeline';
import { useT } from '../../../lib/i18n/I18nContext';

const VIEWPORT_SELECTOR = '[data-slot="aui_thread-viewport"]';
const CONTEXT_CHARS = 24;
const MAX_TIMELINE_EVENTS = 50;

function messageText(message: AssistantState['thread']['messages'][number]): string {
  return message.content
    .flatMap(part => (part.type === 'text' ? [part.text] : []))
    .join('\n');
}

function buildHits(
  messages: readonly AssistantState['thread']['messages'][number][],
  query: string,
  viewport: HTMLElement | null
): SearchHit[] {
  const needle = query.trim().toLowerCase();
  if (needle.length === 0) return [];
  const scrollHeight = viewport?.scrollHeight ?? 0;
  const hits: SearchHit[] = [];
  for (const message of messages) {
    const text = messageText(message);
    if (text.length === 0) continue;
    const haystack = text.toLowerCase();
    let from = 0;
    let occurrence = 0;
    for (;;) {
      const at = haystack.indexOf(needle, from);
      if (at === -1) break;
      const element = viewport?.querySelector<HTMLElement>(`[data-message-id="${message.id}"]`);
      const position =
        element && scrollHeight > 0 ? (element.offsetTop / scrollHeight) * 100 : 0;
      hits.push({
        id: `${message.id}:${occurrence}`,
        before: text.slice(Math.max(0, at - CONTEXT_CHARS), at),
        match: text.slice(at, at + needle.length),
        after: text.slice(at + needle.length, at + needle.length + CONTEXT_CHARS),
        position,
      });
      from = at + needle.length;
      occurrence += 1;
    }
  }
  return hits;
}

function scrollToMessage(messageId: string, viewport: HTMLElement | null) {
  const element = viewport?.querySelector<HTMLElement>(`[data-message-id="${messageId}"]`);
  element?.scrollIntoView({ behavior: 'smooth', block: 'center' });
}

function formatTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return '';
  return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

function buildTimelineEvents(
  messages: readonly AssistantState['thread']['messages'][number][]
): { events: TimelineEvent[]; messageIdByEventId: Map<string, string> } {
  const userMessages = messages.filter(message => message.role === 'user').slice(-MAX_TIMELINE_EVENTS);
  const messageIdByEventId = new Map<string, string>();
  const events = userMessages.map((message, index): TimelineEvent => {
    const eventId = `turn:${message.id}`;
    messageIdByEventId.set(eventId, message.id);
    const text = messageText(message).trim();
    const isLast = index === userMessages.length - 1;
    return {
      id: eventId,
      when: isLast ? 'now' : 'past',
      time: message.createdAt ? formatTime(new Date(message.createdAt).toISOString()) : '',
      title: text.length > 0 ? text.slice(0, 80) : '',
    };
  });
  return { events, messageIdByEventId };
}

/** `Cmd+F` on macOS, `Ctrl+F` elsewhere. Only while focus is inside `container`. */
function useFindShortcut(container: HTMLDivElement | null, onTrigger: () => void) {
  useEffect(() => {
    if (!container) return;
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== 'f') return;
      // `Node.contains` is reflexive, so this also covers focus landing on
      // `container` itself (its own `tabIndex={-1}`).
      if (!container.contains(document.activeElement)) return;
      event.preventDefault();
      onTrigger();
    };
    container.addEventListener('keydown', handler);
    return () => container.removeEventListener('keydown', handler);
  }, [container, onTrigger]);
}

export function ChatConversationMap({ children }: { children: ReactNode }) {
  const { t } = useT();
  const messages = useAuiState((state: AssistantState) => state.thread.messages);
  const containerRef = useRef<HTMLDivElement>(null);
  const [containerEl, setContainerEl] = useState<HTMLDivElement | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [activeIndex, setActiveIndex] = useState(0);
  const [timelineOpen, setTimelineOpen] = useState(false);

  const setContainerRef = useCallback((el: HTMLDivElement | null) => {
    containerRef.current = el;
    setContainerEl(el);
  }, []);

  const viewport = containerEl?.querySelector<HTMLElement>(VIEWPORT_SELECTOR) ?? null;

  const hits = useMemo(() => buildHits(messages, query, viewport), [messages, query, viewport]);
  const { events, messageIdByEventId } = useMemo(() => buildTimelineEvents(messages), [messages]);

  useFindShortcut(containerEl, () => setSearchOpen(true));

  // Reset on the state change that invalidates the previous index, in the
  // event handler that causes it — not in an effect keyed on `query`, which
  // would run a second, avoidable render after the query's own.
  const onQueryChange = useCallback((next: string) => {
    setQuery(next);
    setActiveIndex(0);
  }, []);

  useEffect(() => {
    const hit = hits[activeIndex];
    if (!hit) return;
    const messageId = hit.id.split(':')[0];
    if (messageId) scrollToMessage(messageId, viewport);
  }, [activeIndex, hits, viewport]);

  const onStep = useCallback(
    (delta: number) => {
      if (hits.length === 0) return;
      setActiveIndex(index => (index + delta + hits.length) % hits.length);
    },
    [hits.length]
  );

  const onTimelineClick = useCallback(
    (eventId: string) => {
      const messageId = messageIdByEventId.get(eventId);
      if (messageId) scrollToMessage(messageId, viewport);
      setTimelineOpen(false);
    },
    [messageIdByEventId, viewport]
  );

  return (
    <div
      ref={setContainerRef}
      // Programmatically focusable (not tab-reachable, `-1`) so the
      // `Cmd`/`Ctrl+F` scope check below (`container.contains(document.activeElement)`)
      // has a container-level focus target even when the click/focus that
      // opened this thread landed on a descendant that later unmounts.
      tabIndex={-1}
      className="relative flex h-full min-h-0 w-full flex-col outline-none"
      data-testid="chat-conversation-map">
      {(searchOpen || timelineOpen) && (
        <div className="absolute inset-x-0 top-2 z-20 flex justify-center px-2">
          {searchOpen && (
            <ConversationSearch
              data-testid="chat-conversation-search"
              query={query}
              hits={hits}
              activeIndex={activeIndex}
              onQueryChange={setQuery}
              onStep={onStep}
              placeholder={t('conversations.conversationSearch.placeholder')}
              previousMatchLabel={t('conversations.conversationSearch.previousMatch')}
              nextMatchLabel={t('conversations.conversationSearch.nextMatch')}
            />
          )}
          {timelineOpen && (
            <div data-testid="chat-conversation-timeline" className="ms-2">
              <Timeline
                events={events}
                visibleCount={events.length}
                onClick={event => {
                  // Each event renders as one direct child of the `Timeline`
                  // root (`data-slot="timeline"`), in `events` order — there is
                  // no per-row id in the vendored markup to select on, so the
                  // clicked row's position among its siblings is the row's
                  // index into `events`.
                  const root = event.currentTarget;
                  const index = Array.from(root.children).findIndex(child =>
                    child.contains(event.target as Node)
                  );
                  const clicked = index >= 0 ? events[index] : undefined;
                  if (clicked) onTimelineClick(clicked.id);
                }}
              />
            </div>
          )}
        </div>
      )}
      {children}
      <button
        type="button"
        data-testid="chat-conversation-timeline-toggle"
        aria-label={t('conversations.conversationSearch.timelineToggle')}
        onClick={() => setTimelineOpen(open => !open)}
        className="absolute end-2 top-2 z-20 rounded-full border border-border/60 bg-background px-2 py-1 text-xs text-content-muted hover:text-content-secondary">
        {t('conversations.conversationSearch.timelineToggle')}
      </button>
    </div>
  );
}

export default ChatConversationMap;
