import {
  defineToolkit,
  type ToolCallMessagePartComponent,
  type Toolkit,
} from '@assistant-ui/react';
import { useMemo } from 'react';

import { SubagentCall } from '../components/ChatToolParts';
import { DocumentArtifactCall, MediaGenerationCall } from './MediaAndDocumentCalls';

/**
 * One assistant-ui toolkit entry.
 *
 * Every OpenHuman tool the model can call is executed by the core, never the
 * browser, so every entry is `type: 'backend'` — assistant-ui never tries to
 * run it and never expects `description`/`parameters` from us (those are
 * owned by the core's tool schema, sent to the model over the wire). The only
 * thing an entry contributes on the frontend is *how a call renders*.
 *
 * `display` follows assistant-ui's chain-of-thought convention: `'inline'`
 * (the default) folds the call into the same activity trace as every other
 * tool call; `'standalone'` pulls it out for something that deserves its own
 * spot in the transcript (a generated image, a produced document).
 */
export interface OpenHumanToolEntry {
  type: 'backend';
  display?: 'inline' | 'standalone';
  render: ToolCallMessagePartComponent;
}

/**
 * The toolkit registry, keyed by the tool name the core sends on the wire.
 *
 * A tool name NOT listed here is not an error: assistant-ui falls through to
 * the surface's own `components.ToolFallback` (`ChatToolFallback` in
 * `ChatToolParts.tsx`), which is how every ordinary/dynamic tool (shell, file
 * ops, MCP, Composio, web search, ...) has always rendered and still does —
 * including the approval-gate and `composio_connect` routing, both orthogonal
 * to any one tool's name and therefore not something a per-name registry can
 * own. Only tools whose call deserves its *own* rich element belong here.
 *
 * A function, not a module-level object: `ChatToolParts.tsx` imports from
 * `AssistantUiRuntimeProvider.tsx` (for `useAuiThreadId`), which imports this
 * module (for the toolkit) — a real cycle. Evaluating `SubagentCall` in a
 * module-scope object literal races that cycle: whichever side of it loads
 * first can capture `SubagentCall` before `ChatToolParts.tsx` has finished
 * defining it, baking `undefined` into a frozen entry. Building the record
 * inside a function defers that read to call time, after every module in the
 * cycle has finished loading.
 *
 * To add an entry: import the render component and add a key here. Nothing
 * else in this module needs to change — `buildOpenHumanToolkit` /
 * `useOpenHumanToolkit` pick up every entry automatically.
 */
export function openHumanToolEntries(): Record<string, OpenHumanToolEntry> {
  return {
    /**
     * A sub-agent delegation. Never approval-gated (the orchestrator spawns
     * it directly), so its render skips the gate check every other entry
     * would need and goes straight to the shared delegation card — exactly
     * what the old `ChatToolFallback`'s `toolName === 'task'` branch did
     * before this registry replaced the manual switch.
     */
    task: { type: 'backend', display: 'inline', render: SubagentCall },
  };
}

/**
 * Build the toolkit. `defineToolkit` only types/validates the entries; the
 * object it returns is cheap to recompute, so callers that are not React
 * components (tests, non-hook call sites) can call this directly instead of
 * the hook.
 */
export function buildOpenHumanToolkit(): Toolkit {
  return defineToolkit(openHumanToolEntries());
}

/**
 * The toolkit for the runtime provider's `config` (`AuiConfig({ tools: Tools({
 * toolkit }) })` in {@link AssistantUiRuntimeProvider}). Entries are static —
 * not derived from props or Redux — so the memo never recomputes after the
 * first render.
 */
export function useOpenHumanToolkit(): Toolkit {
  return useMemo(() => buildOpenHumanToolkit(), []);
}
