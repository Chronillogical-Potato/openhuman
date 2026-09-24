import { useState } from 'react';

import { cn } from '../../../components/assistant-ui/lib/utils';
import { composioLogoUrl } from '../../../components/composio/toolkitMeta';
import type { ToolCallPresentation } from './toolPresentation';

/**
 * The glyph for a tool call: the connected app's logo for an integration
 * action (the same Composio-hosted logo the Skills page already shows), or the
 * registry's lucide icon. A logo that fails to load falls back to the icon, so
 * an unknown toolkit never renders a broken image.
 */
export function ToolIcon({
  presentation,
  className,
}: {
  presentation: Pick<ToolCallPresentation, 'icon' | 'integration'>;
  className?: string;
}) {
  const [logoFailed, setLogoFailed] = useState(false);
  const Icon = presentation.icon;
  const integration = presentation.integration;
  if (integration?.known && !logoFailed) {
    return (
      <img
        src={composioLogoUrl(integration.slug)}
        alt=""
        aria-hidden
        data-testid="tool-icon-logo"
        className={cn('size-4 shrink-0 rounded-sm object-contain', className)}
        loading="lazy"
        onError={() => setLogoFailed(true)}
      />
    );
  }
  return <Icon aria-hidden data-testid="tool-icon" className={cn('size-4 shrink-0', className)} />;
}
