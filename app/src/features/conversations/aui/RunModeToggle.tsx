import { LuHammer, LuMap } from 'react-icons/lu';

import { useT } from '../../../lib/i18n/I18nContext';
import { useRunMode } from './useRunMode';

/**
 * Composer action button showing and flipping the thread's plan/build run
 * mode. Mounted in `Conversations.tsx`'s `assistantComposerFooterExtras` —
 * the same icon-action row that already holds the background-processes
 * button — following that button's exact markup/classes rather than
 * introducing a new styled toggle.
 *
 * `/plan` and `/build` slash commands are a different workstream's job; they
 * import {@link useRunMode} directly rather than going through this button.
 *
 * Not yet backed by real core behavior: `openhuman.agent_set_run_mode` /
 * `agent_get_run_mode` and `run_mode_changed` are coded to the wire contract
 * but unimplemented by the core as of this writing.
 */
export function RunModeToggle({ threadId }: { threadId: string }) {
  const { t } = useT();
  const { mode, setMode } = useRunMode(threadId);
  const nextMode = mode === 'plan' ? 'build' : 'plan';
  const label =
    mode === 'plan' ? t('conversations.runMode.plan') : t('conversations.runMode.build');

  return (
    <button
      type="button"
      data-testid="run-mode-toggle"
      data-analytics-id="chat-composer-run-mode-toggle"
      data-run-mode={mode}
      onClick={() => void setMode(nextMode)}
      aria-label={t('conversations.runMode.toggleLabel')}
      title={t('conversations.runMode.toggleLabel')}
      className="flex h-7 items-center gap-1.5 rounded-lg px-2 text-content-muted transition-colors hover:bg-surface-hover hover:text-content-secondary">
      {mode === 'plan' ? <LuMap className="h-4 w-4" /> : <LuHammer className="h-4 w-4" />}
      <span className="text-xs font-medium">{label}</span>
    </button>
  );
}

export default RunModeToggle;
