'use client';

/**
 * assistant-ui's elicitation-form element: a structured human-input request
 * (an MCP server, or a sub-agent's `ask_user_clarification`) with a
 * server/requester label, a message, a set of read-only fields, and
 * Decline / Send.
 *
 * Vendored from the assistant-ui `elements-elicitation-form` registry item
 * (https://r.assistant-ui.com/styles/base-nova/elements-elicitation-form.json).
 * Changes from upstream:
 * - `cn` import path (`@/components/assistant-ui/lib/utils`).
 * - Every user-facing label is now a prop with an English default.
 * - `acceptProps` / `declineProps` slots (same `data-analytics-id` rationale
 *   as `approval-card.tsx`).
 * - `onFieldChange` slot: upstream's fields are read-only display rows; a
 *   plain-text/free-form OpenHuman clarification question needs the user to
 *   actually type an answer before "Send" is meaningful. When supplied, a
 *   `kind: 'text'` field renders an editable input instead of a static
 *   value.
 */
import type { ChangeEvent, ComponentProps } from 'react';
import { CheckIcon, PlugIcon, XIcon } from 'lucide-react';

import { cn } from '@/components/assistant-ui/lib/utils';

import { field, inkButton, mono, paper } from './surfaces';

export type ElicitationState = 'request' | 'accepted' | 'declined';

export interface ElicitationField {
  name: string;
  label: string;
  value: string;
  kind: 'text' | 'choice' | 'toggle';
  options?: readonly string[];
  required?: boolean;
}

export function ElicitationForm({
  server,
  needsInputLabel = 'needs input',
  message,
  fields,
  state,
  onAccept,
  onDecline,
  onFieldChange,
  declineLabel = 'Decline',
  sendLabel = 'Send',
  acceptedLabel = (serverName: string) => `Sent to ${serverName}`,
  declinedLabel = 'Declined',
  acceptProps,
  declineProps,
  className,
  ...props
}: Omit<
  ComponentProps<'div'>,
  | 'children'
  | 'server'
  | 'message'
  | 'fields'
  | 'state'
  | 'onAccept'
  | 'onDecline'
> & {
  server: string;
  needsInputLabel?: string;
  message: string;
  fields: readonly ElicitationField[];
  state: ElicitationState;
  onAccept?: () => void;
  onDecline?: () => void;
  /** Editable text fields when supplied; upstream fields render read-only. */
  onFieldChange?: (name: string, value: string) => void;
  declineLabel?: string;
  sendLabel?: string;
  acceptedLabel?: (server: string) => string;
  declinedLabel?: string;
  acceptProps?: ComponentProps<'button'>;
  declineProps?: ComponentProps<'button'>;
}) {
  return (
    <div
      data-slot="elicitation-form"
      className={cn(paper, 'flex w-full max-w-sm flex-col gap-3.5 rounded-[20px] p-4', className)}
      {...props}
    >
      <div className="flex items-center gap-2.5">
        <span className="bg-foreground/[0.05] text-foreground/45 flex size-7 shrink-0 items-center justify-center rounded-lg">
          <PlugIcon className="size-3.5" />
        </span>
        <span className="min-w-0 flex-1 truncate text-[13.5px] font-medium">{server}</span>
        <span className={cn(mono, 'text-foreground/30 shrink-0')}>{needsInputLabel}</span>
      </div>

      <p className="text-foreground/55 text-xs leading-relaxed">{message}</p>

      <div className="flex flex-col gap-2.5">
        {fields.map(item => (
          <div key={item.name} className="flex flex-col gap-1">
            <span className={cn(mono, 'text-foreground/35')}>
              {item.label}
              {item.required && <span className="text-foreground/25"> *</span>}
            </span>
            {item.kind === 'choice' ? (
              <div className="flex flex-wrap gap-1.5">
                {item.options?.map(option => (
                  <span
                    key={option}
                    className={cn(
                      'rounded-full px-2.5 py-1 text-xs transition-colors',
                      option === item.value
                        ? 'bg-foreground text-background'
                        : cn(field, 'text-foreground/55')
                    )}
                  >
                    {option}
                  </span>
                ))}
              </div>
            ) : item.kind === 'toggle' ? (
              <span className="flex items-center gap-2">
                <span
                  aria-hidden
                  className={cn(
                    'flex h-4 w-7 items-center rounded-full p-0.5 transition-colors duration-200',
                    item.value === 'true' ? 'bg-foreground/80' : 'bg-foreground/15'
                  )}
                >
                  <span
                    className={cn(
                      'bg-background size-3 rounded-full transition-transform duration-200 motion-reduce:transition-none',
                      item.value === 'true' && 'translate-x-3'
                    )}
                  />
                </span>
                <span className="text-foreground/55 text-xs">
                  {item.value === 'true' ? 'On' : 'Off'}
                </span>
              </span>
            ) : onFieldChange && state === 'request' ? (
              <input
                type="text"
                value={item.value}
                onChange={(e: ChangeEvent<HTMLInputElement>) =>
                  onFieldChange(item.name, e.target.value)
                }
                className={cn(
                  field,
                  'text-foreground/80 rounded-lg px-2.5 py-1.5 text-xs outline-none'
                )}
              />
            ) : (
              <span className={cn(field, 'text-foreground/80 rounded-lg px-2.5 py-1.5 text-xs')}>
                {item.value}
              </span>
            )}
          </div>
        ))}
      </div>

      <div className="flex h-8 items-center justify-end gap-2">
        {state === 'request' ? (
          <>
            <button
              type="button"
              onClick={onDecline}
              {...declineProps}
              className={cn(
                'text-foreground/55 hover:bg-foreground/[0.06] hover:text-foreground/90 h-8 rounded-full px-3.5 text-xs font-medium transition-[background-color,color,scale] duration-150 active:scale-[0.96]',
                declineProps?.className
              )}
            >
              {declineLabel}
            </button>
            <button
              type="button"
              onClick={onAccept}
              {...acceptProps}
              className={cn(
                inkButton,
                'flex h-8 items-center rounded-full px-3.5 text-xs font-medium',
                acceptProps?.className
              )}
            >
              {sendLabel}
            </button>
          </>
        ) : (
          <span
            key={state}
            className="fade-in animate-in text-foreground/55 flex items-center gap-2 text-xs duration-300"
          >
            {state === 'accepted' ? (
              <>
                <CheckIcon className="size-3.5 text-emerald-500" />
                {acceptedLabel(server)}
              </>
            ) : (
              <>
                <XIcon className="text-foreground/45 size-3.5" />
                {declinedLabel}
              </>
            )}
          </span>
        )}
      </div>
    </div>
  );
}
