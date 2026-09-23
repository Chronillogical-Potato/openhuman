import { ReasoningTraceText } from '@/components/assistant-ui/elements/reasoning-trace';

/**
 * Ephemeral streaming preview (primary stream or a forked branch). Mirrors the
 * current 120-char tail-slice ticker: the full answer arrives as a durable
 * message on completion, this only signals progress without jumping scroll.
 */
const STREAMING_PREVIEW_CHARS = 120;

export function StreamingTailItem({
  text,
  thinking,
  thinkingStartedAt,
  branch = false,
}: {
  text: string;
  thinking?: string;
  /** Epoch ms of the first thinking delta, for the live elapsed badge. */
  thinkingStartedAt?: number;
  branch?: boolean;
}) {
  const tail = text.slice(-STREAMING_PREVIEW_CHARS);
  const truncated = text.length > STREAMING_PREVIEW_CHARS;
  return (
    <div className="flex justify-start" data-testid={branch ? 'stream-branch' : 'stream-primary'}>
      <div className="max-w-[80%] space-y-1">
        {thinking && thinking.length > 0 ? (
          <ReasoningTraceText
            text={thinking}
            timing={thinkingStartedAt !== undefined ? { startedAt: thinkingStartedAt } : undefined}
            streaming
            data-testid={branch ? 'stream-branch-thinking' : 'stream-primary-thinking'}
          />
        ) : null}
        <div className="rounded-2xl bg-surface-subtle px-4 py-2 font-mono text-sm text-content-secondary">
          {truncated ? '…' : ''}
          {tail}
        </div>
      </div>
    </div>
  );
}
