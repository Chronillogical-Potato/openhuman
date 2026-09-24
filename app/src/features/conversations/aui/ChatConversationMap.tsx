/**
 * The conversation map for one thread: a `Cmd`/`Ctrl+F` find-in-conversation
 * bar (the vendored `conversation-search` element), scoped to this thread's
 * messages. The persistent turn rail is assistant-ui's upstream
 * `ConversationMapAui`, mounted inside `thread.tsx`'s scrolling viewport.
 *
 * Wraps `<Thread />` rather than reaching into `thread.tsx`: the shortcut and
 * the popover are chrome around the transcript, not a slot the message tree
 * itself needs to know about.
 */
import { type AssistantState, useAuiState } from '@assistant-ui/react';
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  ConversationSearch,
  type SearchHit,
} from '../../../components/assistant-ui/elements/conversation-search';
import { useT } from '../../../lib/i18n/I18nContext';

const VIEWPORT_SELECTOR = '[data-slot="aui_thread-viewport"]';
const CONTEXT_CHARS = 24;

function messageText(message: AssistantState['thread']['messages'][number]): string {
  return message.content.flatMap(part => (part.type === 'text' ? [part.text] : [])).join('\n');
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
      const position = element && scrollHeight > 0 ? (element.offsetTop / scrollHeight) * 100 : 0;
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

  const setContainerRef = useCallback((el: HTMLDivElement | null) => {
    containerRef.current = el;
    setContainerEl(el);
  }, []);

  const viewport = containerEl?.querySelector<HTMLElement>(VIEWPORT_SELECTOR) ?? null;

  const hits = useMemo(() => buildHits(messages, query, viewport), [messages, query, viewport]);

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
      {searchOpen && (
        <div className="absolute inset-x-0 top-2 z-20 flex justify-center px-2">
          {searchOpen && (
            <ConversationSearch
              data-testid="chat-conversation-search"
              query={query}
              hits={hits}
              activeIndex={activeIndex}
              onQueryChange={onQueryChange}
              onStep={onStep}
              placeholder={t('conversations.conversationSearch.placeholder')}
              previousMatchLabel={t('conversations.conversationSearch.previousMatch')}
              nextMatchLabel={t('conversations.conversationSearch.nextMatch')}
            />
          )}
        </div>
      )}
      {children}
    </div>
  );
}

export default ChatConversationMap;
