/**
 * One control on an MCP server row: an icon, and a tooltip saying what it
 * does.
 *
 * A row carries up to four controls — connect or disconnect, enable or
 * disable, open, remove — and at labelled-button width they wrap onto a second
 * line and push the row's own information off the screen. Icons fit; icons
 * alone do not *say* anything, which is why the label is mandatory here and is
 * spent twice: as the tooltip for a pointer, and as `aria-label` for a screen
 * reader and for anything driving this page by accessible name. There is no
 * arrangement of this component that produces a control with no name.
 */
import { Loader2, type LucideIcon } from 'lucide-react';

import Button from '../../ui/Button';
import Tooltip from '../../ui/Tooltip';

interface McpIconButtonProps {
  /** What the control does, in the imperative. The tooltip *and* the accessible name. */
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  disabled?: boolean;
  /** Show a spinner in place of the icon while this control's work is in flight. */
  busy?: boolean;
  /** `primary` for the one control a row is waiting on (a server not yet connected). */
  tone?: 'default' | 'primary' | 'destructive';
  testId?: string;
}

const McpIconButton = ({
  label,
  icon: Icon,
  onClick,
  disabled,
  busy,
  tone = 'default',
  testId,
}: McpIconButtonProps) => (
  <Tooltip label={label} side="top" delayMs={200}>
    <Button
      variant={tone === 'primary' ? 'primary' : 'tertiary'}
      tone={tone === 'destructive' ? 'danger' : 'default'}
      size="sm"
      iconOnly
      aria-label={label}
      data-testid={testId}
      disabled={disabled}
      onClick={e => {
        e.stopPropagation();
        onClick();
      }}>
      {busy ? (
        <Loader2 className="size-4 animate-spin" aria-hidden="true" />
      ) : (
        <Icon className="size-4" aria-hidden="true" />
      )}
    </Button>
  </Tooltip>
);

export default McpIconButton;
