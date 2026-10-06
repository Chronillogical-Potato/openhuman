/**
 * The composer's thinking-level control. Sits beside the model trigger and
 * picks the reasoning effort every send asks for (`reasoning_effort` on
 * `openhuman.channel_web_chat`), which the core turns into the provider's
 * reasoning setting. `default` leaves the choice to the provider.
 */
import { useT } from '../../../lib/i18n/I18nContext';

export const REASONING_EFFORTS = ['default', 'none', 'low', 'medium', 'high', 'xhigh'] as const;

export type ReasoningEffortChoice = (typeof REASONING_EFFORTS)[number];

/** Narrows a stored/config value to a known choice; anything else is `default`. */
export function toReasoningEffortChoice(value: string | null | undefined): ReasoningEffortChoice {
  const normalized = (value ?? '').trim().toLowerCase();
  if (normalized === 'off') return 'none';
  if (normalized === 'max') return 'xhigh';
  return (REASONING_EFFORTS as readonly string[]).includes(normalized)
    ? (normalized as ReasoningEffortChoice)
    : 'default';
}

export function ReasoningEffortPicker({
  value,
  onChange,
  disabled = false,
}: {
  value: ReasoningEffortChoice;
  onChange: (value: ReasoningEffortChoice) => void;
  disabled?: boolean;
}) {
  const { t } = useT();
  const label = t('composer.reasoning.label');
  return (
    <select
      data-testid="composer-reasoning-effort"
      data-analytics-id="chat-reasoning-effort"
      aria-label={label}
      title={label}
      value={value}
      disabled={disabled}
      onChange={event => onChange(toReasoningEffortChoice(event.target.value))}
      className="h-7 min-w-0 cursor-pointer rounded-md border-none bg-transparent px-2 text-xs font-medium text-content-muted transition-colors hover:bg-surface-hover hover:text-content focus:outline-none focus-visible:ring-1 focus-visible:ring-line disabled:opacity-50">
      {REASONING_EFFORTS.map(effort => (
        <option key={effort} value={effort}>
          {t(`composer.reasoning.${effort}`)}
        </option>
      ))}
    </select>
  );
}

export default ReasoningEffortPicker;
