/**
 * Composer ↔ host draft sync under fast typing.
 *
 * `ComposerTextBridge` mirrored every keystroke into the host with a sync
 * `setState` inside an effect. Keystrokes are discrete events, so each one's
 * effect — and the update it schedules — ran synchronously, and a burst of them
 * (key-repeat, fast typing, an automated driver) chained those updates past
 * React's nested-update limit: "Maximum update depth exceeded", and the chat
 * surface fell to the error boundary. Reproduced in Brave against the dev build
 * by typing a one-line prompt at driver speed.
 *
 * `flushSync` per character is the jsdom stand-in for that burst: each is a
 * synchronous composer update, exactly what a discrete keystroke produces.
 */
import {
  AssistantRuntimeProvider,
  useAui,
  useAuiState,
  useExternalStoreRuntime,
} from '@assistant-ui/react';
import { act, render, waitFor } from '@testing-library/react';
import { type ReactNode, useLayoutEffect, useState } from 'react';
import { flushSync } from 'react-dom';
import { describe, expect, it } from 'vitest';

import { ComposerTextBridge } from './AssistantUiChat';

type Handles = {
  type: (text: string) => void;
  hostSet: (value: string) => void;
  hostValue: () => string;
  composerText: () => string;
};

function Runtime({ children }: { children: ReactNode }) {
  const runtime = useExternalStoreRuntime({ messages: [], onNew: async () => {} });
  return <AssistantRuntimeProvider runtime={runtime}>{children}</AssistantRuntimeProvider>;
}

function Harness({ handles }: { handles: Partial<Handles> }) {
  const [value, setValue] = useState('');
  const aui = useAui();
  const composerText = useAuiState(s => s.composer.text);
  useLayoutEffect(() => {
    Object.assign(handles, {
      hostSet: setValue,
      hostValue: () => value,
      composerText: () => composerText,
      type: (text: string) => {
        for (let n = 1; n <= text.length; n += 1) {
          flushSync(() => aui.composer.setText(text.slice(0, n)));
        }
      },
    });
  });
  return <ComposerTextBridge value={value} onChange={setValue} />;
}

function mount(): Handles {
  const handles: Partial<Handles> = {};
  render(
    <Runtime>
      <Harness handles={handles} />
    </Runtime>
  );
  return handles as Handles;
}

const PROMPT = 'Check the config, then search for the setting, then explain it.';

describe('ComposerTextBridge', () => {
  it('survives a burst of synchronous keystrokes and the host ends on what was typed', async () => {
    const h = mount();
    act(() => h.type(PROMPT));

    await waitFor(() => expect(h.hostValue()).toBe(PROMPT));
    // The host's lagging echoes never overwrote what was typed after them.
    expect(h.composerText()).toBe(PROMPT);
  });

  it('lets a host-side write (restore, dictation, clear) win', async () => {
    const h = mount();
    act(() => h.type('draft'));
    await waitFor(() => expect(h.hostValue()).toBe('draft'));

    act(() => h.hostSet('restored from history'));
    await waitFor(() => expect(h.composerText()).toBe('restored from history'));

    act(() => h.hostSet(''));
    await waitFor(() => expect(h.composerText()).toBe(''));
  });

  it('keeps typing that lands after a host write', async () => {
    const h = mount();
    act(() => h.hostSet('Hello'));
    await waitFor(() => expect(h.composerText()).toBe('Hello'));

    act(() => h.type('Hello, world'));
    await waitFor(() => expect(h.hostValue()).toBe('Hello, world'));
    expect(h.composerText()).toBe('Hello, world');
  });
});
