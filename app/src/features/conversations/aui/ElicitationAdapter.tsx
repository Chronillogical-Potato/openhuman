'use client';

/**
 * OpenHuman glue over the vendored `elements/elicitation-form.tsx`, for a
 * structured human-input request the run cannot continue past: the top-level
 * `ask_user_clarification` tool call, or (per the assistant-ui-elements plan,
 * WS-B row) a sub-agent's own clarification question once WS-D wires its
 * task-card surface. Kept generic — no thread/Redux dependency baked in — so
 * both call sites can reuse it: the props are exactly the question plus an
 * `onAnswer`/`onDecline` pair, and the caller owns how the answer reaches the
 * run.
 *
 * There is no dedicated "answer this tool call" RPC on the wire today; a
 * clarification is unblocked the same way the sub-agent case already is
 * (`ChatToolParts.tsx`'s `SubagentCall.answer`) — appending an ordinary user
 * turn through the runtime, which the core's orchestrator treats as the
 * clarification reply. `onAnswer`/`onDecline` here are therefore thin: the
 * caller supplies whatever "send this text as the next turn" means for its
 * surface.
 */
import { useState } from 'react';

import {
  ElicitationForm,
  type ElicitationField,
} from '../../../components/assistant-ui/elements/elicitation-form';
import { useT } from '../../../lib/i18n/I18nContext';

export interface ElicitationAdapterProps {
  /** Label for who/what is asking — the agent, or a named sub-agent/server. */
  server: string;
  /** The clarification question itself. */
  message: string;
  /** Whether the run is still waiting on an answer. */
  pending: boolean;
  /** Called with the free-text answer when the user submits it. */
  onAnswer: (answer: string) => void;
  /** Called when the user declines to answer. Omit to hide the button. */
  onDecline?: () => void;
  testId?: string;
  analyticsPrefix?: string;
  className?: string;
}

/** Rendering-only state: `ElicitationForm`'s `state` union, from `pending`. */
function elicitationState(pending: boolean, declined: boolean): 'request' | 'accepted' | 'declined' {
  if (declined) return 'declined';
  return pending ? 'request' : 'accepted';
}

export function ElicitationAdapter({
  server,
  message,
  pending,
  onAnswer,
  onDecline,
  testId,
  analyticsPrefix = 'chat-elicitation',
  className,
}: ElicitationAdapterProps) {
  const { t } = useT();
  const [answer, setAnswer] = useState('');
  const [declined, setDeclined] = useState(false);

  const fields: ElicitationField[] = [
    { name: 'answer', label: t('chat.elicitation.title'), value: answer, kind: 'text' },
  ];

  const submit = () => {
    if (answer.trim().length === 0) return;
    onAnswer(answer.trim());
    setAnswer('');
  };

  return (
    <div data-testid={testId} className={className}>
      <ElicitationForm
        server={server}
        needsInputLabel={t('chat.elicitation.needsInput')}
        message={message}
        fields={fields}
        state={elicitationState(pending, declined)}
        onFieldChange={(_name, value) => setAnswer(value)}
        onAccept={submit}
        onDecline={
          onDecline
            ? () => {
                setDeclined(true);
                onDecline();
              }
            : undefined
        }
        declineLabel={t('chat.elicitation.decline')}
        sendLabel={t('chat.elicitation.send')}
        acceptedLabel={s => t('chat.elicitation.sentTo').replace('{server}', s)}
        declinedLabel={t('chat.elicitation.declined')}
        acceptProps={{ 'data-analytics-id': `${analyticsPrefix}-send` }}
        declineProps={{ 'data-analytics-id': `${analyticsPrefix}-decline` }}
      />
    </div>
  );
}
