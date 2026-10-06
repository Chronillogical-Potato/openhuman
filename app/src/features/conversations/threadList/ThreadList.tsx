import { type RefObject, useMemo, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import type { Thread } from '../../../types/thread';
import { isImeCompositionKeyEvent } from '../Conversations';
import {
  folderBasename,
  groupThreads,
  isThreadPinned,
  type ThreadGroupKey,
  threadMatchesQuery,
} from './groupThreads';

/** i18n key for each section header. */
const GROUP_LABEL_KEYS: Record<ThreadGroupKey, string> = {
  pinned: 'chat.sidebar.group.pinned',
  today: 'chat.sidebar.group.today',
  yesterday: 'chat.sidebar.group.yesterday',
  previous7Days: 'chat.sidebar.group.previous7Days',
  previous30Days: 'chat.sidebar.group.previous30Days',
  older: 'chat.sidebar.group.older',
};

interface ThreadListProps {
  /** Threads visible after the sidebar's search/tab filtering. */
  threads: Thread[];
  selectedThreadId: string | null;
  onCreateThread: () => void;
  /** Select a thread (owns dispatch + message load + route sync). */
  onSelectThread: (threadId: string) => void;
  /** Stable, human-readable title for a thread id. */
  resolveTitle: (threadId: string) => string;
  /** Whether a thread has an agent turn in flight; its title shimmers while true. */
  isThreadRunning?: (threadId: string) => boolean;
  /** Threads whose reply finished while another thread was selected. */
  unreadThreadIds?: ReadonlySet<string>;
  /**
   * Pin state per thread. Defaults to the persisted `pinned` label; the parent
   * passes its own when it applies a pin optimistically ahead of the reload.
   */
  isPinned?: (thread: Thread) => boolean;
  /** Pin or unpin a thread. The pin action is hidden when omitted. */
  onTogglePin?: (thread: Thread, pinned: boolean) => void;
  onRequestDelete: (thread: Thread) => void;
  // Inline title rename — controlled by the parent so the edit state stays
  // co-located with the rest of the panel's thread state.
  editingThreadId: string | null;
  editTitleValue: string;
  editTitleInputRef: RefObject<HTMLInputElement | null>;
  onEditTitleValueChange: (value: string) => void;
  onStartEditTitle: (threadId: string) => void;
  onCommitTitle: (threadId: string) => void;
  onCancelEditTitle: () => void;
  onBlurTitle: (threadId: string) => void;
}

/**
 * The conversations left rail: a section header with the "new conversation"
 * affordance docked on the right, above the scrollable thread list with inline
 * rename + delete. Presentational, driven entirely by props so it can be reused
 * by the page and sidebar shells.
 */
export function ThreadList({
  threads,
  selectedThreadId,
  onCreateThread,
  onSelectThread,
  resolveTitle,
  isThreadRunning,
  unreadThreadIds,
  isPinned = isThreadPinned,
  onTogglePin,
  onRequestDelete,
  editingThreadId,
  editTitleValue,
  editTitleInputRef,
  onEditTitleValueChange,
  onStartEditTitle,
  onCommitTitle,
  onCancelEditTitle,
  onBlurTitle,
}: ThreadListProps) {
  const { t } = useT();
  const [query, setQuery] = useState('');
  const visibleThreads = useMemo(
    () => threads.filter(thread => threadMatchesQuery(resolveTitle(thread.id), query)),
    [threads, query, resolveTitle]
  );
  // Recomputed per render on purpose: a list left open overnight should move
  // yesterday's rows out of "Today" on the next update without a timer.
  const groups = groupThreads(visibleThreads, new Date(), isPinned);
  const renderRow = (thread: Thread) => {
    const running = Boolean(isThreadRunning?.(thread.id));
    const unread = !running && Boolean(unreadThreadIds?.has(thread.id));
    const pinned = isPinned(thread);
    return (
      <div
        key={thread.id}
        data-testid={`thread-row-${thread.id}`}
        // The working folder rides on the row as a native tooltip rather than a
        // second line, so the list keeps one `h-8` rhythm.
        title={
          thread.actionDir
            ? t('chat.sidebar.workingFolder').replace('{folder}', folderBasename(thread.actionDir))
            : undefined
        }
        data-analytics-id="chat-sidebar-thread-row"
        role="button"
        tabIndex={0}
        onClick={() => onSelectThread(thread.id)}
        onKeyDown={e => {
          if (e.target !== e.currentTarget) return;
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onSelectThread(thread.id);
          }
        }}
        // A rounded pill per row, separated by spacing rather than
        // hairlines — six dividers in a short list read as a table, not a
        // list of destinations. Alpha fills so the row lifts identically
        // whether the list is projected into the (translucent) app sidebar
        // or rendered inside the opaque chat aside.
        // Fixed `h-8` matching SidebarNav's rows: the hover-revealed
        // actions are taller than the title's line box, so a padding-sized
        // row would grow 4px the moment the pointer entered it and the
        // whole list would shift under the cursor.
        className={`group flex h-8 w-full flex-none cursor-pointer items-center rounded-md px-3 text-left transition-colors ${
          selectedThreadId === thread.id
            ? 'bg-surface/70'
            : 'hover:bg-surface/40 dark:hover:bg-surface/60'
        }`}>
        <div className="flex w-full min-w-0 items-center gap-1.5">
          {editingThreadId === thread.id ? (
            <input
              ref={editTitleInputRef}
              value={editTitleValue}
              onClick={e => e.stopPropagation()}
              onChange={e => onEditTitleValueChange(e.target.value)}
              onKeyDown={e => {
                e.stopPropagation();
                // Ignore the Enter that confirms an IME composition
                // candidate (CJK input) so it doesn't prematurely commit.
                if (isImeCompositionKeyEvent(e)) return;
                if (e.key === 'Enter') {
                  e.preventDefault();
                  onCommitTitle(thread.id);
                } else if (e.key === 'Escape') {
                  // Escape is an explicit cancel — suppress the commit the
                  // ensuing blur would otherwise fire.
                  onCancelEditTitle();
                }
              }}
              onBlur={() => onBlurTitle(thread.id)}
              aria-label={t('chat.editThreadTitle')}
              data-testid={`thread-title-input-${thread.id}`}
              className="h-5 min-w-0 flex-1 border-b border-primary-400 bg-transparent py-0 text-xs font-medium leading-none text-content-secondary outline-hidden"
              autoFocus
            />
          ) : (
            <p
              data-running={running ? 'true' : undefined}
              aria-busy={running || undefined}
              className={`truncate flex-1 text-[14px] ${
                selectedThreadId === thread.id ? 'font-semibold text-content' : 'text-content-muted'
              } ${running ? 'shimmer motion-reduce:animate-none' : ''}`}>
              {resolveTitle(thread.id)}
            </p>
          )}
          {/* Unread only while idle: a running row already says "something is
              happening" through its shimmer, and a dot beside it would claim a
              finished reply that does not exist yet. Hidden on hover so the
              trailing actions take its slot instead of crowding it. */}
          {unread && (
            <span
              data-testid={`thread-unread-${thread.id}`}
              role="img"
              aria-label={t('chat.sidebar.unread')}
              title={t('chat.sidebar.unread')}
              className="h-1.5 w-1.5 flex-none rounded-full bg-primary-500 group-hover:hidden"
            />
          )}
          {onTogglePin && (
            <button
              type="button"
              data-testid={`thread-pin-${thread.id}`}
              data-analytics-id={pinned ? 'chat-sidebar-unpin-thread' : 'chat-sidebar-pin-thread'}
              onClick={e => {
                e.stopPropagation();
                onTogglePin(thread, !pinned);
              }}
              aria-label={pinned ? t('chat.sidebar.unpinThread') : t('chat.sidebar.pinThread')}
              aria-pressed={pinned}
              title={pinned ? t('chat.sidebar.unpinThread') : t('chat.sidebar.pinThread')}
              className={`hidden h-5 w-5 flex-none items-center justify-center rounded transition-colors hover:bg-surface/60 group-hover:inline-flex ${
                pinned ? 'text-primary-500' : 'text-content-faint hover:text-primary-500'
              }`}>
              <svg
                className="h-3 w-3"
                fill={pinned ? 'currentColor' : 'none'}
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-hidden="true">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M16 3l5 5-3 1-4 4 1 5-2 2-4-4-5 5v-1l4-5-4-4 2-2 5 1 4-4z"
                />
              </svg>
            </button>
          )}
          <button
            type="button"
            data-analytics-id="chat-sidebar-edit-thread-title"
            onClick={e => {
              e.stopPropagation();
              onStartEditTitle(thread.id);
            }}
            aria-label={t('chat.editThreadTitle')}
            title={t('chat.editThreadTitle')}
            // `hidden`, not `opacity-0`: the title gets the full row width
            // until hover reveals the trailing actions.
            className="hidden h-5 w-5 flex-none items-center justify-center rounded text-content-faint transition-colors hover:bg-surface/60 hover:text-primary-500 group-hover:inline-flex">
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"
              />
            </svg>
          </button>
          <button
            type="button"
            data-analytics-id="chat-sidebar-delete-thread"
            onClick={e => {
              e.stopPropagation();
              onRequestDelete(thread);
            }}
            className="hidden h-5 w-5 flex-none items-center justify-center rounded text-content-faint transition-colors hover:bg-surface/60 hover:text-coral-500 group-hover:inline-flex"
            title={t('chat.deleteThread')}>
            <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </button>
        </div>
      </div>
    );
  };
  return (
    // Card background / rounded corners come from TwoPanelLayout's pane styling.
    <div className="h-full flex flex-col">
      {/* Pinned above the scroller, not inside it. It used to be the list's
          first child and scrolled away with the threads, so on any account with
          more than a screenful of conversations the one control that starts a
          new one was reachable only by scrolling back to the top.

          The wrapper is `overflow-hidden` purely to carry the same
          `scrollbar-gutter` as the list below it. A gutter is only reserved on
          a scroll container, and `overflow: hidden` makes an element one
          (programmatically scrollable) without it ever scrolling — so on
          Windows, where the bar is laid out in flow and the gutter is real,
          this row keeps the exact left edge the thread pills have. On macOS and
          Linux the bar overlays, the gutter is inert, and both are simply
          `px-2`. Without this the row would sit a scrollbar's width left of
          every pill under it, on one platform only.

          `scrollbar-width:thin` has to come with it. The gutter's width is the
          bar's width, so declaring the gutter without matching the list's
          `thin` reserves a FULL-width band here against a thin one below —
          which showed up as this row rendering visibly narrower than the
          thread pills on both sides, the mirror image of the bug the gutter is
          here to prevent.

          `pb-2` makes the 8px gap below the button match the chat separator's
          8px lower margin above it, so the action is optically centred between
          the primary navigation and the first conversation. */}
      <div className="flex flex-none flex-col gap-1.5 overflow-hidden px-2 pb-2 [scrollbar-gutter:stable_both-edges] [scrollbar-width:thin]">
        {/* "New conversation" as a row, not a header icon. It is the same
          affordance as a thread row — pick a conversation to work in — so it
          takes the same shape: `h-8` pill, same radius, same hover fill, same
          14px label, sitting in the same column. As a 20px icon docked in a
          section header it was both the smallest hit target in the sidebar
          and the only control there that did not look like the thing it
          produced. That header is gone with it: it was a group label for a
          list that is already the only thing in its region, under a separator
          that already divides it from the nav above.

          A `<button>` rather than a `div[role=button]` like the thread rows:
          those rows carry nested action buttons (rename, delete) and cannot
          legally nest a button inside a button, which is why they hand-roll
          the role and key handling. This row has no children, so it can be
          the real element and get Enter/Space, focus and semantics for free.

          Outline, not filled: a solid accent button would make the loudest
          thing in the sidebar an action nobody needs most of the time, and it
          would outrank the selected conversation, which is the one row that
          should carry emphasis. A border states the affordance and leaves
          `text-content-muted` matching an unselected thread row. The border
          uses the same `content-faint` token as the composer's outline, so
          the two read as one edge language rather than two.

          The accent arrives on HOVER, and only on the BORDER and a 10% fill.
          The label stays neutral (`content-secondary`, the same lift a thread
          row gets). Taking the text primary too was tried and reads as a link
          rather than a button: three accented properties at once made the row
          the loudest thing in the sidebar on hover, which is the exact failure
          the resting state is designed to avoid one paragraph above. The `+`
          follows the label through `currentColor`, so the edge and the fill
          carry the accent on their own. That keeps the resting state as quiet as the
          argument above requires while making the row unmistakably the
          actionable one the moment it is pointed at. The `+` inherits it for
          free through `currentColor`.

          `justify-between` puts the label on the left edge with the `+` pushed
          to the right, rather than the two sitting together at the start. The
          glyph then lands in the row's own trailing gutter, where a thread
          row's hover actions appear, so the column has one consistent right
          edge instead of an icon floating mid-row.
 */}
        <button
          type="button"
          data-testid="new-thread-button"
          data-analytics-id="chat-sidebar-new-thread"
          onClick={onCreateThread}
          title={t('chat.newThreadShortcut')}
          className="group flex h-8 w-full flex-none cursor-pointer items-center justify-between gap-1.5 rounded-md border border-content-faint/35 px-3 text-left text-[14px] text-content-muted transition-colors hover:border-primary-500/60 hover:bg-primary-500/10 hover:text-content-secondary">
          <span className="truncate">{t('chat.newConversation')}</span>
          <svg
            className="h-3.5 w-3.5 flex-none"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            aria-hidden="true">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
        </button>
        {/* Title filter. Same pill geometry as the rows so it sits in their
            column; a bare input with no border until focus, because at rest it
            is the least important control here. Escape clears it — the one
            keystroke anyone tries first to get the full list back. Filtering
            is title-only: message search is the command palette's job. */}
        <div className="relative flex h-8 items-center">
          <svg
            className="pointer-events-none absolute left-3 h-3.5 w-3.5 text-content-faint"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
            aria-hidden="true">
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M21 21l-4.35-4.35M10.5 18a7.5 7.5 0 100-15 7.5 7.5 0 000 15z"
            />
          </svg>
          <input
            type="search"
            data-testid="thread-search-input"
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Escape' && query) {
                e.preventDefault();
                e.stopPropagation();
                setQuery('');
              }
            }}
            placeholder={t('chat.sidebar.searchPlaceholder')}
            aria-label={t('chat.sidebar.searchPlaceholder')}
            className="h-8 w-full rounded-md border border-transparent bg-surface/40 pl-8 pr-7 text-[13px] text-content placeholder:text-content-faint outline-hidden transition-colors focus:border-content-faint/35 dark:bg-surface/60 [&::-webkit-search-cancel-button]:hidden"
          />
          {query && (
            <button
              type="button"
              data-testid="thread-search-clear"
              data-analytics-id="chat-sidebar-clear-search"
              onClick={() => setQuery('')}
              aria-label={t('chat.sidebar.clearSearch')}
              title={t('chat.sidebar.clearSearch')}
              className="absolute right-1.5 inline-flex h-5 w-5 items-center justify-center rounded text-content-faint transition-colors hover:bg-surface/60 hover:text-content-secondary">
              <svg
                className="h-3 w-3"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
                aria-hidden="true">
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M6 18L18 6M6 6l12 12"
                />
              </svg>
            </button>
          )}
        </div>
      </div>
      {/* Rows carry no padding gutter of their own — a thread pill spans the
          full width the scroll container gives it, so its hover/selected fill
          reads as the width of the list rather than a floating inset card, with
          `px-2` breathing it an equal 8px off either edge.

          `scrollbar-width` makes the bar an OVERLAY here, and it is doing so by
          opting this one pane OUT of the app-wide rules rather than by adding
          anything. `index.css` paints every pane's bar with `::-webkit-scrollbar`
          at a fixed 10px whose track is permanently reserved (only the thumb's
          colour animates — toggling `width` would reflow the pane on every
          scroll), so a full-bleed list silently lost 10px on the right the
          moment it overflowed. That stylesheet's own comment records the escape
          hatch: a standard `scrollbar-*` property takes precedence and disables
          the `::-webkit-scrollbar` styling entirely. The runtime is Wry as of
          #5456 (`crates/openhuman-app/Cargo.toml` enables the `wry` feature; the CEF
          notes around it are historical), so on macOS/Linux WebKit that hands
          the pane back the platform's native overlay bar — zero reserved width,
          fading on its own, which is what the `data-scrolling` machinery in
          `lib/autoHideScrollbars.ts` exists to imitate everywhere else.

          `scrollbar-gutter` stays for the platform where that is not true.
          Windows WebView2 is Chromium and still lays a classic bar out in flow;
          per spec a gutter is ignored for overlay bars, so the declaration is
          inert on macOS and reserves a matched band on both sides on Windows.
          The pill is therefore symmetric on every platform and never resizes as
          the list crosses the overflow threshold — it is simply 8px inset where
          the bar overlays and 8px + the bar's width where it does not.

          Only `scrollbar-width` is set, not `scrollbar-color`: colouring the
          thumb is what tips WebKit out of overlay mode and back into a laid-out
          bar, which would undo the whole point. Native overlay bars already
          track the platform's light/dark appearance.

          Vertical rhythm is `gap` on the column, not a margin on each row — a
          margin also lands after the last row and pads the scroll floor
          unevenly against `pb-3`. */}
      <div className="flex flex-1 flex-col gap-0.5 overflow-y-auto px-2 pb-3 [scrollbar-gutter:stable_both-edges] [scrollbar-width:thin]">
        {threads.length === 0 ? (
          <p className="px-4 py-6 text-xs text-content-faint text-center">{t('chat.noThreads')}</p>
        ) : groups.length === 0 ? (
          <p
            data-testid="thread-search-empty"
            className="px-4 py-6 text-xs text-content-faint text-center">
            {t('chat.sidebar.noMatches')}
          </p>
        ) : (
          // Each section is a labelled group so screen readers announce
          // "Today, list" etc. The header is a plain muted caption, not a
          // control: sections are not collapsible, so they should not look it.
          groups.map(group => (
            <section
              key={group.key}
              data-testid={`thread-group-${group.key}`}
              aria-label={t(GROUP_LABEL_KEYS[group.key])}
              className="flex flex-col gap-0.5">
              <h3 className="px-3 pb-0.5 pt-2 text-[11px] font-medium uppercase tracking-wide text-content-faint first:pt-0">
                {t(GROUP_LABEL_KEYS[group.key])}
              </h3>
              {group.threads.map(renderRow)}
            </section>
          ))
        )}
      </div>
    </div>
  );
}
