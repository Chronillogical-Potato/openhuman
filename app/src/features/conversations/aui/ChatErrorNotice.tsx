import { type AssistantState, useAuiState } from '@assistant-ui/react';

import { GuardrailNotice } from '../../../components/assistant-ui/elements/guardrail-notice';
import { useT } from '../../../lib/i18n/I18nContext';
import { CHAT_ERROR_METADATA_KEY } from '../../../store/threadSlice';

/** Mirrors the Rust `GuardrailPayload` carried on `chat_error` (wire-contract.md). */
interface GuardrailReasonLike {
  code: string;
  message: string;
}
interface GuardrailPayloadLike {
  verdict: string;
  score: number;
  reasons: GuardrailReasonLike[];
}
interface ChatErrorMetadata {
  errorType?: string;
  guardrail?: GuardrailPayloadLike;
}

const selectChatError = (s: AssistantState): ChatErrorMetadata | undefined => {
  const custom = s.message.metadata?.custom as
    | { extraMetadata?: Record<string, unknown> }
    | undefined;
  return custom?.extraMetadata?.[CHAT_ERROR_METADATA_KEY] as ChatErrorMetadata | undefined;
};

/**
 * Renders the vendored `GuardrailNotice` for a message whose turn failed
 * with `chat_error{error_type:"guardrail"}` (wire-contract.md). The message's
 * plain-text content is suppressed for exactly this case in
 * `toThreadMessageLike` (`assistantUiMessages.ts`), so this card is the only
 * thing that message renders — mounted unconditionally in `AssistantMessage`
 * (`thread.tsx`), it renders `null` for every other message.
 *
 * No "try instead" alternatives exist on the wire today (`GuardrailPayload`
 * carries only `verdict`/`score`/`reasons`), so `alternatives` is always
 * empty and the element hides that section — this only shows the policy tag
 * and the reasons the guardrail cited.
 */
export function ChatErrorNotice() {
  const chatError = useAuiState(selectChatError);
  const { t } = useT();
  if (chatError?.errorType !== 'guardrail' || !chatError.guardrail) return null;
  const { guardrail } = chatError;
  const explanation =
    guardrail.reasons.map(reason => reason.message).join(' ') ||
    t('conversations.chatError.guardrail.explanationFallback');
  return (
    <GuardrailNotice
      data-testid="assistant-ui-guardrail-notice"
      title={t('conversations.chatError.guardrail.title')}
      explanation={explanation}
      policy={guardrail.verdict}
      alternatives={[]}
      alternativesLabel={t('conversations.chatError.guardrail.tryInstead')}
    />
  );
}
