/**
 * `useFollowBottom` — the contract, at unit level.
 *
 * The e2e specs cover the integrated behaviour but they missed a real defect:
 * the turn-start alignment (`scrollIntoView({ block: 'start' })`) scrolls UP,
 * which the follow listener read as the reader leaving the bottom. E2E did not
 * catch it because the listener checks proximity first, and after a SHORT user
 * message the viewport is still within the 80px threshold — so the bug only
 * appears for a user message tall enough to push past it. A browser test would
 * need a fixture sized to that boundary; here the geometry is set directly.
 *
 * jsdom performs no layout, so every metric is assigned explicitly and the
 * observable is the imperative `scrollTo` call, not a resulting `scrollTop` —
 * the same approach as `thread.openScroll.test.tsx`.
 */
import { AssistantUiRuntimeProvider } from '@/providers/AssistantUiRuntimeProvider';
import chatRuntimeReducer from '@/store/chatRuntimeSlice';
import threadReducer, { loadThreadMessages } from '@/store/threadSlice';
import type { ThreadMessage } from '@/types/thread';
import { configureStore } from '@reduxjs/toolkit';
import { render } from '@testing-library/react';
import { act } from 'react';
import { Provider } from 'react-redux';
import { afterEach, beforeEach, describe, expect, it, type MockInstance, vi } from 'vitest';

import { Thread } from './thread';

vi.mock('@/services/api/threadApi', () => ({
  threadApi: { getDerivedTranscript: vi.fn().mockResolvedValue({ items: [], nextCursor: null }) },
}));

function msg(id: string, sender: 'user' | 'agent', content: string): ThreadMessage {
  return {
    id,
    content,
    type: 'text',
    extraMetadata: {},
    sender,
    createdAt: `2026-09-23T00:00:${id.padStart(2, '0')}Z`,
  };
}

function makeStore() {
  return configureStore({ reducer: { thread: threadReducer, chatRuntime: chatRuntimeReducer } });
}

/**
 * Capture every `ResizeObserver` callback the tree installs, so content growth
 * can be simulated. The shared setup polyfills a no-op observer, which would
 * make these tests vacuously green — nothing would ever fire.
 */
let resizeCallbacks: ResizeObserverCallback[] = [];
let scrollToSpy: MockInstance;
const RealResizeObserver = globalThis.ResizeObserver;

beforeEach(() => {
  resizeCallbacks = [];
  globalThis.ResizeObserver = class {
    constructor(cb: ResizeObserverCallback) {
      resizeCallbacks.push(cb);
    }
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  scrollToSpy = vi.spyOn(HTMLElement.prototype, 'scrollTo').mockImplementation(() => {});
  vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(() => {});
});

afterEach(() => {
  globalThis.ResizeObserver = RealResizeObserver;
  vi.restoreAllMocks();
});

function viewportOf(container: HTMLElement): HTMLElement {
  const el = container.querySelector<HTMLElement>('[data-slot="aui_thread-viewport"]');
  if (!el) throw new Error('viewport not rendered');
  return el;
}

/** jsdom reports 0 for every box, so the reader's position is set outright. */
function setGeometry(el: HTMLElement, scrollTop: number, scrollHeight: number, clientHeight = 500) {
  Object.defineProperty(el, 'scrollHeight', { value: scrollHeight, configurable: true });
  Object.defineProperty(el, 'clientHeight', { value: clientHeight, configurable: true });
  el.scrollTop = scrollTop;
}

/**
 * The reader scrolling: an input that can scroll (a wheel notch here), then the
 * `scroll` event it produced. A bare `scroll` event is a layout shift or
 * someone else's programmatic scroll, not the reader.
 */
function readerScrolls(viewport: HTMLElement) {
  act(() => {
    viewport.dispatchEvent(new WheelEvent('wheel', { deltaY: -100 }));
    viewport.dispatchEvent(new Event('scroll'));
  });
}

/** Fire every captured resize callback, as a height change would. */
function growContent() {
  act(() => {
    for (const cb of resizeCallbacks) cb([], {} as ResizeObserver);
  });
}

function followedBottom(viewport: HTMLElement): boolean {
  return scrollToSpy.mock.calls.some((call, i) => {
    if (scrollToSpy.mock.contexts[i] !== viewport) return false;
    const options = call[0] as ScrollToOptions | undefined;
    return options?.top === viewport.scrollHeight;
  });
}

function renderThread() {
  const store = makeStore();
  act(() => {
    store.dispatch({
      type: loadThreadMessages.fulfilled.type,
      payload: { threadId: 't1', messages: [msg('1', 'user', 'hi'), msg('2', 'agent', 'hello')] },
    });
  });
  const { container } = render(
    <Provider store={store}>
      <AssistantUiRuntimeProvider threadId="t1">
        <Thread />
      </AssistantUiRuntimeProvider>
    </Provider>
  );
  return { container, store, viewport: viewportOf(container) };
}

describe('useFollowBottom', () => {
  it('follows content growth for a reader at the bottom', () => {
    const { viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));
    scrollToSpy.mockClear();

    setGeometry(viewport, 500, 1400);
    growContent();

    expect(followedBottom(viewport)).toBe(true);
  });

  it('stops following once the reader scrolls up', () => {
    const { viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    // Up, and past the 80px threshold — a deliberate move into history.
    setGeometry(viewport, 100, 1000);
    readerScrolls(viewport);
    scrollToSpy.mockClear();

    setGeometry(viewport, 100, 1400);
    growContent();

    expect(followedBottom(viewport)).toBe(false);
  });

  it('resumes following when the reader returns to the bottom', () => {
    const { viewport } = renderThread();
    // Start AT the bottom and scroll away, so following is genuinely off before
    // the return is tested. Going straight to 100 from the initial baseline of
    // 0 is an INCREASE, which never trips the clearing branch — the test would
    // then pass with the proximity branch deleted.
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    setGeometry(viewport, 100, 1000);
    readerScrolls(viewport);

    setGeometry(viewport, 500, 1000);
    readerScrolls(viewport);
    scrollToSpy.mockClear();

    setGeometry(viewport, 500, 1400);
    growContent();

    expect(followedBottom(viewport)).toBe(true);
  });

  it('keeps following after the turn-start alignment scrolls the viewport UP', () => {
    // `ThreadBottomFollower` aligns a new user message with
    // `scrollIntoView({ block: 'start' })`. For a reader at the bottom that
    // LOWERS `scrollTop` — the message sits above the trailing margin and the
    // status slot — which is indistinguishable from the reader moving into
    // history unless the follower claims its own scroll.
    //
    // Ordering is the whole point: the alignment is synchronous, its `scroll`
    // event is not. Setting the follow flag inside the effect is not enough,
    // because the later event sees a drop against a stale baseline and clears
    // it again. `claimScroll` re-baselines while `scrollTop` already holds the
    // post-alignment value, so that event becomes a no-op.
    //
    // The e2e suite cannot reach this without a fixture sized to the 80px
    // boundary: with a short user message the viewport stays within the
    // threshold and the proximity branch masks it.
    const { store, viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    // Make the alignment behave like the real one: `scrollIntoView` on a
    // message above the trailing margin LOWERS `scrollTop`, synchronously,
    // inside the effect. A no-op mock cannot reproduce the ordering this test
    // exists to check — `claimScroll` would capture the pre-alignment value
    // and the test would pass for the wrong reason.
    vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(() => {
      viewport.scrollTop = 150;
    });

    act(() => {
      store.dispatch({
        type: loadThreadMessages.fulfilled.type,
        payload: {
          threadId: 't1',
          messages: [
            msg('1', 'user', 'hi'),
            msg('2', 'agent', 'hello'),
            msg('3', 'user', 'a tall follow-up question'),
          ],
        },
      });
    });

    // The alignment's own scroll event arrives now.
    act(() => viewport.dispatchEvent(new Event('scroll')));
    scrollToSpy.mockClear();

    // The reply streams.
    setGeometry(viewport, 150, 1600);
    growContent();

    expect(followedBottom(viewport)).toBe(true);
  });

  it('ignores the growth that prompted a claim, so the alignment survives', () => {
    // The user message grows the content box, which queues a resize
    // notification; the alignment then claims the scroll. Following that queued
    // callback would scroll to the bottom and erase the alignment before it is
    // painted — making it pointless rather than short-lived.
    const { store, viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    vi.spyOn(Element.prototype, 'scrollIntoView').mockImplementation(() => {
      viewport.scrollTop = 150;
    });

    // The new user message has already grown the box to 1400 when the alignment
    // runs and claims at that height. `scrollTop` must put the reader AT the
    // bottom (1400 - 500 clientHeight = 900), or `ThreadBottomFollower` returns
    // early on its own proximity guard and never aligns or claims at all —
    // which would make this test pass for the wrong reason.
    setGeometry(viewport, 900, 1400);
    act(() => {
      store.dispatch({
        type: loadThreadMessages.fulfilled.type,
        payload: {
          threadId: 't1',
          messages: [
            msg('1', 'user', 'hi'),
            msg('2', 'agent', 'hello'),
            msg('3', 'user', 'a tall follow-up question'),
          ],
        },
      });
    });
    scrollToSpy.mockClear();

    // The queued notification for THAT growth must not move the viewport.
    growContent();
    expect(followedBottom(viewport)).toBe(false);

    // The reply arriving is growth beyond the claim, and is followed.
    setGeometry(viewport, 150, 1900);
    growContent();
    expect(followedBottom(viewport)).toBe(true);
  });

  it('keeps following when scrollTop drops with no reader input behind it', () => {
    // A disclosure collapsing, content shrinking as parts are swapped, and
    // assistant-ui's `useScrollLock` writing an old `scrollTop` back all lower
    // `scrollTop` past the threshold with nobody touching anything. Each used
    // to read as the reader leaving and stop following mid-turn.
    const { viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    setGeometry(viewport, 100, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));
    scrollToSpy.mockClear();

    setGeometry(viewport, 100, 1400);
    growContent();

    expect(followedBottom(viewport)).toBe(true);
  });

  it('stops following on a keyboard scroll up', () => {
    const { viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    setGeometry(viewport, 100, 1000);
    act(() => {
      viewport.ownerDocument.dispatchEvent(new KeyboardEvent('keydown', { key: 'PageUp' }));
      viewport.dispatchEvent(new Event('scroll'));
    });
    scrollToSpy.mockClear();

    setGeometry(viewport, 100, 1400);
    growContent();

    expect(followedBottom(viewport)).toBe(false);
  });

  it('does not treat a growth-only scroll event as the reader leaving', () => {
    const { viewport } = renderThread();
    setGeometry(viewport, 500, 1000);
    act(() => viewport.dispatchEvent(new Event('scroll')));

    // The reply grew by more than the threshold and a queued `scroll` runs
    // before the resize callback. `scrollTop` has NOT moved, so this must not
    // clear the flag even though the distance now reads far from the bottom.
    setGeometry(viewport, 500, 1400);
    act(() => viewport.dispatchEvent(new Event('scroll')));
    scrollToSpy.mockClear();

    growContent();

    expect(followedBottom(viewport)).toBe(true);
  });
});
