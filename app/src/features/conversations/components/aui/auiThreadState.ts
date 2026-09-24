import { type AssistantState, useAuiState } from '@assistant-ui/react';
import debugFactory from 'debug';

const debug = debugFactory('openhuman:assistant-ui:transcript');

/**
 * Optional-safe reads of the assistant-ui runtime for the CURRENT subtree.
 *
 * Two properties make these hooks usable from the transcript, and both are
 * load-bearing:
 *
 * 1. **They read the runtime from React context**, never from
 *    `state.thread.selectedThreadId`. `AssistantUiRuntimeProvider` is
 *    thread-parameterized and the `Thread` is mounted by two hosts — the home
 *    chat (follows the selection) and `WorkflowCopilotPanel` (its own nested
 *    runtime on a dedicated builder thread). Reading the selection here would
 *    paint the home chat's state inside the copilot.
 *
 * 2. **They tolerate the runtime being absent.** Every selector goes through
 *    `s.optional.<scope>`, which resolves to `undefined` rather than throwing
 *    when no `AuiProvider` is above the component (assistant-ui's default
 *    client throws on a direct `s.thread` read), so a component using them
 *    stays mountable in a test or preview without a runtime.
 *
 * Selectors are module-level constants: `useAuiState` keys its internal
 * memoization on selector identity, so an inline arrow would re-subscribe the
 * underlying `useSyncExternalStore` on every render of the chat's hot path.
 */

const selectCanEdit = (s: AssistantState) => s.optional.thread?.capabilities.edit;

const selectCanSwitchToBranch = (s: AssistantState) =>
  s.optional.thread?.capabilities.switchToBranch;

const selectCanReload = (s: AssistantState) => s.optional.thread?.capabilities.reload;

/**
 * Whether the mounted runtime's adapter can honour message editing and the
 * branch picker.
 *
 * `useOpenHumanExternalStore` supplies `onEdit` (via the `threads.edit_message`
 * RPC, core workstream C4) and `setMessages` (a no-op stub — the core has no
 * per-branch message model yet, so `onEdit`/`onReload` both truncate the
 * thread's single lineage rather than forking one). Supplying either key at
 * all is what turns assistant-ui's `capabilities.edit` /
 * `capabilities.switchToBranch` on, so this hook reports both true whenever a
 * runtime is mounted and false only when none is (a test/preview host with no
 * `AuiProvider` above it).
 *
 * Both affordances in `components/assistant-ui/thread.tsx` are gated on this
 * (#5897): `UserMessage` renders the vendored `EditMessage` element only when
 * `canEdit`, and `BranchPicker` returns `null` unless `canSwitchToBranch`. The
 * gate stays in place rather than being deleted now that both are wired: an
 * edit button that looks supported and silently does nothing is worse than no
 * button, and the day the adapter regresses (loses `onEdit`/`setMessages`)
 * this hook is what turns the affordance back off automatically.
 */
export function useAuiEditCapabilities(): { canEdit: boolean; canSwitchToBranch: boolean } {
  const canEdit = useAuiState(selectCanEdit) ?? false;
  const canSwitchToBranch = useAuiState(selectCanSwitchToBranch) ?? false;
  if (canEdit || canSwitchToBranch) {
    debug(
      '[assistant-ui] transcript capabilities changed edit=%s branch=%s',
      canEdit,
      canSwitchToBranch
    );
  }
  return { canEdit, canSwitchToBranch };
}

/**
 * Whether the runtime can re-run an assistant turn.
 *
 * Same defect class as Edit and BranchPicker, and it bites harder: assistant-ui
 * computes the Reload button's disabled state from
 * `isRunning || isDisabled || role !== 'assistant'` and never consults
 * `capabilities.reload`, so the button is *enabled* on every settled assistant
 * message. `useOpenHumanExternalStore` supplies no `onReload`, and the runtime
 * throws `Runtime does not support reloading messages.` on click.
 *
 * Gated rather than deleted, so the affordance appears by itself the day the
 * adapter grows `onReload`.
 */
export function useAuiReloadCapability(): boolean {
  return useAuiState(selectCanReload) ?? false;
}

/**
 * THE EDIT / BRANCH SEAM.
 *
 * When the core gains a branch model and `useOpenHumanExternalStore` grows
 * `onEdit` + `setMessages`, two affordances become renderable and both belong
 * in the assistant-ui message components (`components/assistant-ui/thread.tsx`),
 * NOT here:
 *
 * - an edit composer, gated on `useAuiEditCapabilities().canEdit`, rendered
 *   from `ComposerPrimitive.Root` / `ComposerPrimitive.Input` inside a
 *   `MessagePrimitive.Root` for that turn;
 * - `BranchPickerPrimitive.Root` / `.Previous` / `.Number` / `.Count` /
 *   `.Next`, gated on `canSwitchToBranch`, rendered alongside the turn's
 *   existing copy / react / share action row.
 *
 * They are deliberately absent rather than rendered-and-inert: an edit button
 * that looks supported and silently does nothing is worse than no button.
 *
 * That rule was stated here but not enforced anywhere until #5897 — this hook
 * had zero production consumers while `ActionBarPrimitive.Edit` shipped
 * unconditionally, so the button was rendered, clickable and inert. The gate is
 * wired now; keep it wired when the affordances land in `thread.tsx`.
 */
export const EDIT_AND_BRANCH_SEAM = Object.freeze({
  editComposer: 'thread.tsx UserMessage — gated on useAuiEditCapabilities().canEdit',
  branchPicker: 'thread.tsx BranchPicker — gated on useAuiEditCapabilities().canSwitchToBranch',
});
