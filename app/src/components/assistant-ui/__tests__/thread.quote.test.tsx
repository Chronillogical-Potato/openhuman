/**
 * Quote-a-selection, proven from the component `/chat` actually mounts.
 *
 * `quote.tsx` was written, reviewed and then imported by nothing except
 * `pages/dev/assistant-ui-demo`. Mounting in that demo proves nothing about the
 * real route, which is the whole reason the audit that found it counted
 * dev-only components separately. So this test drives `Thread` — the component
 * `AssistantUiChat` renders — and never touches `BaseDemo`.
 */
import {
  AssistantRuntimeProvider,
  type ThreadMessageLike,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { Thread } from '../thread';

const messages: ThreadMessageLike[] = [
  { role: 'user', content: [{ type: 'text', text: 'explain mitochondria' }] },
  {
    role: 'assistant',
    content: [{ type: 'text', text: 'The mitochondria is the powerhouse of the cell.' }],
  },
];

function Harness({ children }: { children?: ReactNode }) {
  const runtime = useExternalStoreRuntime({
    messages,
    isRunning: false,
    convertMessage: (m: ThreadMessageLike) => m,
    onNew: async () => {},
  });
  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <Thread model={null} onModelChange={() => {}} />
      {children}
    </AssistantRuntimeProvider>
  );
}

/**
 * Put a non-collapsed selection over `node` and tell the document about it.
 *
 * The toolbar listens on `document` for `mouseup`, reads `window.getSelection()`
 * inside a `requestAnimationFrame`, and walks up for the `data-message-id` that
 * `MessagePrimitive.Root` emits — so the stub has to be anchored at a real node
 * inside the rendered message, not at a detached one.
 */
function selectWithin(node: Node, text: string) {
  const removeAllRanges = vi.fn();
  vi.spyOn(window, 'getSelection').mockReturnValue({
    isCollapsed: false,
    anchorNode: node,
    focusNode: node,
    rangeCount: 1,
    toString: () => text,
    getRangeAt: () => ({
      commonAncestorContainer: node,
      getBoundingClientRect: () => ({ top: 100, left: 40, width: 120, height: 18 }),
    }),
    removeAllRanges,
  } as unknown as Selection);
  fireEvent.mouseUp(document);
  return { removeAllRanges };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('quoting a selection from the live Thread', () => {
  it('offers a Quote button over a selection inside a message', async () => {
    render(<Harness />);
    const answer = await screen.findByText('The mitochondria is the powerhouse of the cell.');

    expect(document.querySelector('[data-slot="selection-toolbar"]')).toBeNull();
    selectWithin(answer, 'powerhouse of the cell');

    await waitFor(() =>
      expect(document.querySelector('[data-slot="selection-toolbar"]')).not.toBeNull()
    );
  });

  it('drops the excerpt into the composer when Quote is clicked', async () => {
    render(<Harness />);
    const answer = await screen.findByText('The mitochondria is the powerhouse of the cell.');

    // Nothing quoted yet, so the composer shows no preview.
    expect(document.querySelector('[data-slot="composer-quote"]')).toBeNull();

    selectWithin(answer, 'powerhouse of the cell');
    const quoteButton = await waitFor(() => {
      const el = document.querySelector('[data-slot="selection-toolbar-quote"]');
      if (!el) throw new Error('no quote button');
      return el;
    });
    fireEvent.click(quoteButton);

    const preview = await waitFor(() => {
      const el = document.querySelector('[data-slot="composer-quote"]');
      if (!el) throw new Error('no composer quote preview');
      return el;
    });
    // The excerpt itself, not just the chrome — a preview that renders empty
    // would satisfy a presence-only assertion while showing the user nothing.
    expect(preview.textContent).toContain('powerhouse of the cell');
  });

  it('dismisses the quote from the composer', async () => {
    render(<Harness />);
    const answer = await screen.findByText('The mitochondria is the powerhouse of the cell.');
    selectWithin(answer, 'powerhouse of the cell');
    fireEvent.click(
      await waitFor(() => {
        const el = document.querySelector('[data-slot="selection-toolbar-quote"]');
        if (!el) throw new Error('no quote button');
        return el;
      })
    );
    await waitFor(() =>
      expect(document.querySelector('[data-slot="composer-quote"]')).not.toBeNull()
    );

    fireEvent.click(screen.getByLabelText('Dismiss quote'));

    await waitFor(() => expect(document.querySelector('[data-slot="composer-quote"]')).toBeNull());
  });

  it('offers no Quote button for a selection outside any message', async () => {
    // The guard that keeps the toolbar from appearing over the composer, the
    // thread list, or anything else on the page.
    render(
      <Harness>
        <p data-testid="outside">not part of a message</p>
      </Harness>
    );
    const outside = await screen.findByTestId('outside');

    selectWithin(outside, 'not part of a message');

    await new Promise(resolve => setTimeout(resolve, 50));
    expect(document.querySelector('[data-slot="selection-toolbar"]')).toBeNull();
  });
});
