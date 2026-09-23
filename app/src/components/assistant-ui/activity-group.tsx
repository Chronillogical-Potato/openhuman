'use client';

import {
  ToolGroupContent,
  ToolGroupRoot,
  ToolGroupTrigger,
} from '@/components/assistant-ui/tool-group';
import { type MessagePrimitive, useAuiState } from '@assistant-ui/react';
import { type FC, type PropsWithChildren, useState } from 'react';

export type ActivityGroupPart = MessagePrimitive.GroupedParts.GroupPart;

/**
 * Trigger text for a run of reasoning and tool calls.
 *
 * Tool calls are what the reader counts; reasoning is either there or not, so
 * it is named rather than counted. The group only exists when it holds at
 * least one of the two, so the empty fallback is never shown in practice.
 */
export function activityGroupLabel(reasoningCount: number, toolCount: number): string {
  const tools = toolCount > 0 ? `${toolCount} tool ${toolCount === 1 ? 'call' : 'calls'}` : null;
  if (reasoningCount > 0 && tools) return `Reasoning · ${tools}`;
  if (reasoningCount > 0) return 'Reasoning';
  return tools ?? 'Activity';
}

/**
 * One disclosure for everything the agent did between the user's input and its
 * answer: reasoning and tool calls together, in the order they happened.
 *
 * It replaces a chain-of-thought wrapper that split the same run into separate
 * reasoning and tool groups. A turn that alternates — think, call, think, call —
 * then rendered as a stack of unrelated collapsibles, each with its own trigger,
 * and the answer drowned among them. As one group the message reads input →
 * work → answer however the work interleaved.
 *
 * Open while the work is live (`running`, or `requires-action` for a tool
 * parked on an approval, whose decision card lives inside), closed once it
 * settles so the answer leads. The first manual toggle wins from then on.
 */
export const ActivityGroup: FC<PropsWithChildren<{ group: ActivityGroupPart }>> = ({
  group,
  children,
}) => {
  const { indices } = group;
  // Numbers, not arrays, so the selectors are stable across renders.
  const toolCount = useAuiState(
    s => indices.filter(i => s.message.parts[i]?.type === 'tool-call').length
  );
  const reasoningCount = useAuiState(
    s => indices.filter(i => s.message.parts[i]?.type === 'reasoning').length
  );
  const [userOpen, setUserOpen] = useState<boolean | null>(null);

  const running = group.status.type === 'running';
  const live = running || group.status.type === 'requires-action';

  return (
    <ToolGroupRoot
      variant="ghost"
      open={userOpen ?? live}
      onOpenChange={setUserOpen}>
      <ToolGroupTrigger
        count={toolCount}
        label={activityGroupLabel(reasoningCount, toolCount)}
        active={running}
      />
      <ToolGroupContent>{children}</ToolGroupContent>
    </ToolGroupRoot>
  );
};
