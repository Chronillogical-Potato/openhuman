/**
 * The chat composer's `/` command and `@` mention pickers.
 *
 * Mounted by `thread.tsx` through the `ComposerTriggers` slot (inside
 * `ComposerPrimitive.Unstable_TriggerPopoverRoot`), so both sources run under
 * the assistant-ui runtime (`useAui`, `useAuiState`) and read the thread the
 * runtime provider is bound to. Rendering is the vendored assistant-ui
 * `ComposerTriggerPopover`; this file only spreads the two sources into it.
 */
import { ComposerTriggerPopover } from '@/components/assistant-ui/composer-trigger-popover';
import { useAuiThreadId } from '@/providers/AssistantUiRuntimeProvider';

import { useMentionSource } from './useMentionSource';
import { useSlashCommandSource } from './useSlashCommandSource';

export function ComposerTriggers() {
  const threadId = useAuiThreadId();
  const slash = useSlashCommandSource(threadId);
  const mention = useMentionSource(threadId);
  return (
    <>
      <ComposerTriggerPopover char="/" data-testid="composer-slash-popover" {...slash} />
      <ComposerTriggerPopover char="@" data-testid="composer-mention-popover" {...mention} />
    </>
  );
}
