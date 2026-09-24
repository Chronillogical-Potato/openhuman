'use client';

/**
 * Composer-header "live artifact deck" adapter over the vendored
 * `elements/artifact-card.tsx` `ArtifactCard`, replacing the deleted
 * `components/chat/ArtifactCard.tsx`.
 *
 * Only artifacts with NO owning tool call (`ArtifactSnapshot.toolCallId`
 * absent) reach this deck — `Conversations.tsx`'s `liveArtifactDeck` filters
 * those out; an artifact with a `toolCallId` renders inline through its own
 * tool-call card instead (`MediaAndDocumentCalls.tsx`). A `ready` artifact
 * never reaches either: it moves to `ChatFilesChip` (unchanged).
 *
 * The vendored card has no built-in failed/retry state (it only knows
 * "writing" vs. "settled"), so the failed case renders the settled meta line
 * with the failure reason and an explicit Retry button underneath, wired to
 * the same `aiRegenerate` re-dispatch the legacy card used.
 */
import { FileTextIcon, ImageIcon, PresentationIcon } from 'lucide-react';
import type { ElementType } from 'react';

import { ArtifactCard } from '../../../components/assistant-ui/elements/artifact-card';
import { formatFileSize } from '../../../lib/attachments';
import { useT } from '../../../lib/i18n/I18nContext';
import type { ArtifactSnapshot } from '../../../store/chatRuntimeSlice';
import { Button } from '../../../components/ui';

const KIND_ICONS: Record<ArtifactSnapshot['kind'], ElementType> = {
  presentation: PresentationIcon,
  document: FileTextIcon,
  image: ImageIcon,
  other: FileTextIcon,
};

export interface ArtifactCardAdapterProps {
  artifact: ArtifactSnapshot;
  /** When provided, render a Retry affordance on the `failed` state. */
  onRetry?: (artifactId: string) => void;
}

export function ArtifactCardAdapter({ artifact, onRetry }: ArtifactCardAdapterProps) {
  const { t } = useT();
  const generating = artifact.status === 'in_progress';
  const meta =
    artifact.status === 'ready' && artifact.sizeBytes != null
      ? `${t('chat.artifact.ready')} · ${formatFileSize(artifact.sizeBytes)}`
      : artifact.status === 'failed'
        ? t('chat.artifact.failed')
        : '';

  return (
    <div className="flex flex-col items-start gap-1.5" data-testid="artifact-card-adapter">
      <ArtifactCard
        title={artifact.title}
        meta={meta}
        generating={generating}
        words={0}
        writingLabel={t('conversations.tools.working')}
        icon={KIND_ICONS[artifact.kind]}
      />
      {artifact.status === 'failed' && onRetry ? (
        <Button
          variant="secondary"
          size="xs"
          analyticsId="chat-artifact-retry"
          onClick={() => onRetry(artifact.artifactId)}>
          {t('chat.artifact.retry')}
        </Button>
      ) : null}
    </div>
  );
}

export default ArtifactCardAdapter;
