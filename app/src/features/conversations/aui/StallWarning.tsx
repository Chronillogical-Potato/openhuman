/**
 * The "this turn has gone quiet" banner above the composer (after OpenClaw's
 * health chips): the phase-specific explanation, how long the turn has been
 * silent, and Stop right beside it so the user does not have to hunt for the
 * composer's stop button. Informational only — the turn keeps running and the
 * banner clears itself on the next inference signal.
 */
import { useT } from '../../../lib/i18n/I18nContext';
import { formatElapsed } from '../components/AssistantUiToolCall';
import { useLiveElapsed } from './useLiveElapsed';

export interface StallWarningProps {
  /** `inferenceStatusByThread.phase` of the quiet turn. */
  phase: string;
  /** Epoch ms of the turn's last inference signal. */
  quietSince: number;
  onStop: () => void;
}

/** `75_000` → "1m 15s", rounded to whole seconds for a banner. */
function formatQuiet(ms: number): string {
  return formatElapsed(Math.round(ms / 1000) * 1000);
}

export function StallWarning({ phase, quietSince, onStop }: StallWarningProps) {
  const { t } = useT();
  const quietMs = useLiveElapsed(quietSince, true) ?? 0;
  return (
    <div className="mb-2 flex items-start justify-between gap-3" role="status">
      <p
        className="text-xs text-amber-700"
        data-testid="chat-stall-warning"
        data-chat-stall-phase={phase}>
        {t(phase === 'thinking' ? 'chat.stallWarning.thinking' : 'chat.stallWarning.working')}{' '}
        <span data-testid="chat-stall-quiet" className="tabular-nums font-medium">
          {t('chat.status.quietFor').replace('{duration}', formatQuiet(quietMs))}
        </span>
      </p>
      <button
        type="button"
        data-analytics-id="chat-stall-stop"
        data-testid="chat-stall-stop"
        onClick={onStop}
        className="flex-none rounded-md border border-amber-600/40 px-2 py-0.5 text-xs font-medium text-amber-700 transition-colors hover:bg-amber-500/10">
        {t('chat.status.stop')}
      </button>
    </div>
  );
}

export default StallWarning;
