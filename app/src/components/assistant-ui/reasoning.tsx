'use client';

/**
 * The per-part reasoning renderer (markdown, no chrome). The thread renders a
 * run of reasoning parts through `OpenHumanReasoningGroup`
 * (`reasoning-group.tsx`), which is built on assistant-ui's static
 * `ReasoningPanel` (`elements/reasoning-panel.tsx`) — titled steps, a live
 * "Thinking… Ns" trigger and a settled "Thought for Ns" label. This part
 * renderer only remains for a host-supplied `ReasoningGroup` override that
 * wants the raw per-part children.
 */
import { MarkdownText } from '@/components/assistant-ui/markdown-text';
import type { ReasoningMessagePartComponent } from '@assistant-ui/react';
import { memo } from 'react';

const ReasoningImpl: ReasoningMessagePartComponent = () => <MarkdownText />;

const Reasoning = memo(ReasoningImpl) as unknown as ReasoningMessagePartComponent;

Reasoning.displayName = 'Reasoning';

export { Reasoning };
