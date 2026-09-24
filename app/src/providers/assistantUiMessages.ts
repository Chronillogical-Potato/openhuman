import type {
  ThreadAssistantMessagePart,
  ThreadMessageLike,
  ThreadUserMessagePart,
  ToolApprovalOption,
} from '@assistant-ui/react';

import { parseMessageImages } from '../lib/attachments';
import { unwrapToolCallEnvelope } from '../lib/chat/toolCallEnvelope';
import {
  isActiveTimelineStatus,
  type PendingApproval,
  type ProcessingTranscriptItem,
  type StreamingAssistantState,
  type ToolTimelineEntry,
} from '../store/chatRuntimeSlice';
import { extractAgentSources } from '../utils/toolTimelineFormatting';
import {
  FEEDBACK_METADATA_KEY,
  FEEDBACK_ROW_IDS_METADATA_KEY,
  type MessageFeedback,
} from '../store/threadSlice';
import type { ThreadMessage } from '../types/thread';

/**
 * Redux -> assistant-ui message mapping.
 *
 * assistant-ui is adopted as a *runtime* (semantics + API), never as a store:
 * `chatRuntimeSlice` and `threadSlice` remain the single source of truth for
 * messages, streaming, tool state, queueing and persistence. Everything here is
 * a pure, read-only projection of that state onto the shape the runtime wants.
 * Nothing in this module writes.
 *
 * The one property that matters for performance is stated as a test, not a
 * comment: converting the transcript while a token streams must not re-convert
 * the settled messages above the live tail. `ChatThreadView.renderPerf.test.tsx`
 * pins the equivalent property for the render tree; `assistantUiMessages.test.ts`
 * pins it for this projection.
 */

type ConversionCacheEntry = {
  timeline: readonly ToolTimelineEntry[];
  transcript: readonly ProcessingTranscriptItem[];
  converted: ThreadMessageLike;
};

/**
 * Cache keyed on the source message and its persisted process arrays. Socket
 * tokens only replace the live tail, so a settled message converts exactly
 * once while its transcript/timeline identities remain stable.
 */
const conversionCache = new WeakMap<ThreadMessage, ConversionCacheEntry>();

const EMPTY_TIMELINE: readonly ToolTimelineEntry[] = [];
const EMPTY_TRANSCRIPT: readonly ProcessingTranscriptItem[] = [];

const RECOVERED_TOOL_NAMES_KEY = 'assistantUiToolNames';

/**
 * The rating persisted on a message, when it is one of the two values the
 * runtime accepts.
 *
 * `extraMetadata` is untyped JSON from disk, so this narrows rather than casts:
 * a stale or hand-edited value must render as "unrated" instead of reaching the
 * runtime as a bad `submittedFeedback`.
 */
function persistedFeedback(msg: ThreadMessage): MessageFeedback | undefined {
  const value = msg.extraMetadata?.[FEEDBACK_METADATA_KEY];
  return value === 'positive' || value === 'negative' ? value : undefined;
}

/** Synthetic id for the live streaming tail. Stable so React reconciles it. */
export const STREAMING_TAIL_ID = '__openhuman_streaming_tail__';

/**
 * Convert one persisted message.
 *
 * Agent content is passed through `unwrapToolCallEnvelope` for the same reason
 * the transcript renderer does it: a `{content, tool_calls}` provider envelope
 * must never reach a rendered surface as raw JSON. Tool *activity* is not
 * projected as assistant-ui tool-call parts — it lives in the far richer
 * `toolTimelineByThread` projection that `ToolTimelineBlock` renders, and
 * duplicating it here would paint every tool twice.
 */
function jsonObject(value: unknown): Record<string, never> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
  try {
    return JSON.parse(JSON.stringify(value)) as Record<string, never>;
  } catch {
    return {};
  }
}

function toolArgs(entry: ToolTimelineEntry): Record<string, never> {
  if (!entry.argsBuffer) return {};
  try {
    return jsonObject(JSON.parse(entry.argsBuffer));
  } catch {
    return { raw: entry.argsBuffer } as never;
  }
}

/**
 * The `result` payload for a settled non-sub-agent tool part.
 *
 * assistant-ui's tool-call part has no status field, so a terminal status has
 * to travel inside `result` or not at all. It did not travel: the adapter fell
 * back to `result !== undefined`, which reads as success, and a failed or
 * cancelled tool rendered with a "done" label and a check.
 *
 * A tool that produced no output already reported `{ status, failure }` here,
 * so only the failed-*with*-output case needed a shape — `value` carries the
 * real output beside the status, and {@link isToolStatusEnvelope} unwraps it.
 * The success path is byte-identical to before, deliberately: every reader of
 * a successful result keeps seeing exactly what it saw.
 */
function toolResultPayload(entry: ToolTimelineEntry): unknown {
  const terminalFailure = entry.status === 'error' || entry.status === 'cancelled';
  if (!terminalFailure) return entry.result ?? { status: entry.status, failure: entry.failure };
  return {
    status: entry.status,
    failure: entry.failure,
    ...(entry.result !== undefined ? { value: entry.result } : {}),
  };
}

/**
 * Presentation data that rides a tool part's `artifact`.
 *
 * assistant-ui's tool-call part has no slot for a display label, a duration
 * or a structured result, and this adapter used to drop all three, so the
 * chat card fell back to guessing a label from the tool name and arguments.
 * `artifact` is the part's UI-only field, which is exactly this.
 */
export interface OpenHumanToolArtifact {
  kind: 'openhuman-tool';
  /** Server label, for dynamic tools the client registry cannot describe. */
  displayName?: string;
  detail?: string;
  elapsedMs?: number;
  structured?: unknown;
}

export function readOpenHumanToolArtifact(value: unknown): OpenHumanToolArtifact | undefined {
  if (!value || typeof value !== 'object') return undefined;
  return (value as { kind?: unknown }).kind === 'openhuman-tool'
    ? (value as OpenHumanToolArtifact)
    : undefined;
}

function toolArtifact(entry: ToolTimelineEntry): OpenHumanToolArtifact | undefined {
  const artifact: OpenHumanToolArtifact = {
    kind: 'openhuman-tool',
    ...(entry.displayName ? { displayName: entry.displayName } : {}),
    ...(entry.detail ? { detail: entry.detail } : {}),
    ...(entry.elapsedMs !== undefined ? { elapsedMs: entry.elapsedMs } : {}),
    ...(entry.structured !== undefined ? { structured: entry.structured } : {}),
  };
  return Object.keys(artifact).length > 1 ? artifact : undefined;
}

function toolPart(entry: ToolTimelineEntry): ThreadAssistantMessagePart {
  const running = isActiveTimelineStatus(entry.status);
  const isSubagent = entry.name.startsWith('subagent:') || entry.subagent !== undefined;
  const args = isSubagent
    ? jsonObject({
        subagent_type: entry.subagent?.agentId ?? entry.name.replace(/^subagent:/, ''),
        description: entry.detail,
        ...(running ? { progress: entry.subagent } : {}),
      })
    : toolArgs(entry);

  return {
    type: 'tool-call',
    toolCallId: entry.id,
    toolName: isSubagent ? 'task' : entry.name,
    args,
    argsText: JSON.stringify(args, null, 2),
    ...(!isSubagent && toolArtifact(entry) ? { artifact: toolArtifact(entry) } : {}),
    ...(!running
      ? {
          result: isSubagent
            ? (entry.subagent ?? { status: entry.status })
            : toolResultPayload(entry),
        }
      : {}),
  };
}

/**
 * The decision set offered for a parked tool call.
 *
 * Each `id` is verbatim the `decision` literal `openhuman.approval_decide`
 * takes, so the runtime hands the adapter an option id it can forward to the
 * RPC without a translation table — see `useOpenHumanExternalStore`. The
 * `kind`s are what assistant-ui resolves to the boolean `approved` it reports
 * alongside the id, and what a renderer keys its default labels off.
 *
 * `reject-always` is deliberately absent: the core has no "never allow this
 * tool" decision, and offering one that silently degrades to a one-off deny
 * would be a lie about what the click did.
 */
export const APPROVAL_DECISION_OPTIONS: readonly ToolApprovalOption[] = [
  { id: 'approve_once', kind: 'allow-once' },
  { id: 'approve_always_for_tool', kind: 'allow-always' },
  { id: 'deny', kind: 'reject-once' },
];

/**
 * `toolCallId` prefix for the part synthesised when a parked approval cannot be
 * matched to a live timeline row. Namespaced so it can never collide with a
 * core-issued tool-call id (the duplicate-id invariant above is a hard one).
 */
const APPROVAL_PART_ID_PREFIX = '__openhuman_approval__:';

/** The part the parked call is asking about, when no timeline row carries it. */
function syntheticApprovalPart(approval: PendingApproval): ThreadAssistantMessagePart {
  // `command` is the redacted command/path/url the gate extracted for display;
  // rendering it as the call's args is what makes the prompt answerable.
  const args = approval.command ? { command: approval.command } : {};
  return {
    type: 'tool-call',
    toolCallId: `${APPROVAL_PART_ID_PREFIX}${approval.requestId}`,
    toolName: approval.toolName,
    args: args as Record<string, never>,
    argsText: JSON.stringify(args, null, 2),
    approval: { id: approval.requestId, options: APPROVAL_DECISION_OPTIONS },
  };
}

/**
 * Hang a parked approval off the tool part it is gating.
 *
 * The `approval_request` socket event carries no `tool_call_id` (see
 * `ChatApprovalRequestEvent`), so the row is matched by name against the
 * newest still-unsettled call — a `result` means the call already ran and
 * cannot be the one parked. When nothing matches (the progress channel is
 * bounded and can drop the `tool_call` frame, and the gate can park before the
 * frame lands at all) a part is synthesised rather than dropped: a prompt in
 * the wrong visual slot is recoverable, a turn that parks with no prompt at all
 * is the bug this exists to close.
 */
function withApproval(
  parts: ThreadAssistantMessagePart[],
  approval: PendingApproval
): ThreadAssistantMessagePart[] {
  const index = parts.reduce(
    (best, part, at) =>
      part.type === 'tool-call' && part.toolName === approval.toolName && part.result === undefined
        ? at
        : best,
    -1
  );
  if (index < 0) return [...parts, syntheticApprovalPart(approval)];
  return parts.map((part, at) =>
    at === index
      ? { ...part, approval: { id: approval.requestId, options: APPROVAL_DECISION_OPTIONS } }
      : part
  );
}

/**
 * A reasoning part, carrying the block's timing (epoch ms) under
 * `providerMetadata.openhuman` when it is known. `OpenHumanReasoningGroup`
 * reads it back (`reasoningTimingOf`) to show "Thinking… Ns" while the block
 * streams and "Thought for Ns" once it settles. Blocks recorded before timing
 * existed carry none and settle to a plain "Thought".
 */
export function reasoningPart(
  text: string,
  startedAt: number | undefined,
  endedAt: number | undefined
): ThreadAssistantMessagePart {
  if (startedAt === undefined && endedAt === undefined) return { type: 'reasoning', text };
  const timing: { startedAt?: number; endedAt?: number } = {};
  if (startedAt !== undefined) timing.startedAt = startedAt;
  if (endedAt !== undefined) timing.endedAt = endedAt;
  return { type: 'reasoning', text, providerMetadata: { openhuman: timing } };
}

/**
 * Project one assistant message into assistant-ui parts.
 *
 * The surface carries the turn as it happened: the agent's reasoning as a
 * COLLAPSED disclosure, every tool row in issue order, then the answer.
 *
 * ## What is here, and what stays in the rail
 *
 * Reasoning (`kind: 'thinking'`) renders inline again, through the static
 * reasoning panel: collapsed by default to one "Thought for Ns" line, expanded
 * into titled steps while it streams, so a turn that thought for ten seconds
 * shows one quiet line rather than ten seconds of prose — which is what made it
 * clutter the first time round.
 *
 * Narration is deliberately NOT restored. It is the turn's running commentary,
 * it duplicates the answer on the final round, and it is the bulk of what made
 * the old surface a firehose. It stays in `processingByThread` and renders in
 * the process rail behind {@link TurnProcessTrail}, which is unchanged.
 *
 * ## Ordering
 *
 * The transcript is the ordered record of the turn and is walked in array
 * order. Timeline rows the transcript never names — a legacy snapshot has no
 * transcript at all, and a live turn can mint a row before its pointer lands —
 * are merged in by their own `seq` rather than appended after everything else,
 * which is what previously let a row the agent issued FIRST render last.
 *
 * The two `seq` fields are NOT one ordering space, whatever
 * `PersistedToolTimelineEntry.seq`'s doc comment says: a transcript item's
 * `seq` is its index in the transcript array (`chatRuntimeSlice.ts`, the
 * `toolCall` push uses `seq: list.length`) while a timeline row's comes from
 * the per-thread `toolTimelineSeqByThread` counter. So the merge below compares
 * timeline `seq` to timeline `seq` only, never across the two.
 *
 * The answer text is appended last, and that is correct rather than merely
 * convenient: it is the persisted `msg.content`, i.e. what the agent said when
 * it had finished, so nothing it produced can belong after it. Anything the
 * agent said BEFORE a tool call is narration, which lives in the rail.
 *
 * **Every tool part must have a distinct `toolCallId`.** assistant-ui keys them
 * as `toolCallId-${id}` and *throws* on a repeat ("Duplicate key … in
 * useResources"), which takes the whole thread render down rather than dropping
 * a row — so this is a hard invariant, not a tidiness rule, and it is enforced
 * here at the boundary as well as at each producer. `claim` guards every emit
 * below; the sources upstream (the live Redux slice and the derived transcript
 * mapper) also mint unique ids, but threads persisted before those fixes still
 * carry colliding ones.
 */
function assistantParts(
  text: string,
  timeline: readonly ToolTimelineEntry[],
  transcript: readonly ProcessingTranscriptItem[]
): ThreadAssistantMessagePart[] {
  const parts: ThreadAssistantMessagePart[] = [];
  const timelineById = new Map(timeline.map(entry => [entry.id, entry]));
  const emittedToolIds = new Set<string>();
  const claim = (entry: ToolTimelineEntry): boolean => {
    if (emittedToolIds.has(entry.id)) return false;
    emittedToolIds.add(entry.id);
    return true;
  };

  // Rows the transcript names. Two pointers can resolve to the same row (a
  // provider that emits tool calls without ids writes the empty string for all
  // of them), which is why this is a set of resolved row ids rather than a
  // count of pointers.
  const referenced = new Set<string>();
  for (const item of transcript) {
    if (item.kind !== 'toolCall') continue;
    const entry = timelineById.get(item.callId);
    if (entry) referenced.add(entry.id);
  }

  // Rows with no pointer, oldest first. These are merged into the walk below
  // rather than appended after it.
  const unreferenced = timeline
    .filter(entry => !referenced.has(entry.id))
    .sort((a, b) => a.seq - b.seq);
  let nextUnreferenced = 0;
  /** Emit every unreferenced row the agent issued before `seq` (all of them at the end). */
  const drainBefore = (seq: number | null) => {
    while (nextUnreferenced < unreferenced.length) {
      const entry = unreferenced[nextUnreferenced];
      if (entry === undefined || (seq !== null && entry.seq >= seq)) break;
      nextUnreferenced += 1;
      if (claim(entry)) parts.push(toolPart(entry));
    }
  };

  for (const item of transcript) {
    if (item.kind === 'thinking') {
      if (item.text.trim().length > 0) {
        parts.push(reasoningPart(item.text, item.startedAt, item.endedAt));
      }
      continue;
    }
    // Narration is the turn explaining itself; it stays in the rail.
    if (item.kind !== 'toolCall') continue;
    const entry = timelineById.get(item.callId);
    if (!entry) continue;
    drainBefore(entry.seq);
    if (claim(entry)) parts.push(toolPart(entry));
  }
  drainBefore(null);

  if (text.length > 0) parts.push({ type: 'text', text });
  // `extractAgentSources` is the one place a model-supplied URL is admitted
  // (http(s) only), so sources are derived through it rather than here.
  for (const source of extractAgentSources([...timeline])) {
    parts.push({
      type: 'source',
      sourceType: 'url',
      id: source.id,
      url: source.url,
      title: source.title,
    });
  }
  return parts;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

function requestIdOf(message: ThreadMessage): string | undefined {
  const requestId = message.extraMetadata?.requestId;
  return typeof requestId === 'string' && requestId.length > 0 ? requestId : undefined;
}

function isGenericToolName(name: string): boolean {
  return ['', 'tool', 'unknown', 'unknown_tool'].includes(name.trim().toLowerCase());
}

function recoverTimelineToolNames(
  timeline: readonly ToolTimelineEntry[],
  recoveredNames: readonly string[]
): readonly ToolTimelineEntry[] {
  if (recoveredNames.length === 0 || !timeline.some(entry => isGenericToolName(entry.name))) {
    return timeline;
  }
  // Advance only when a name is actually consumed. `recoveredNames` comes from
  // tool-call envelopes, so it is not positionally aligned with the whole
  // timeline: incrementing on every entry made `[read_file, tool, tool]` +
  // `[web_search, web_fetch]` mis-assign `web_fetch` to the first generic row
  // and leave the second one named `tool`.
  let recoveredIndex = 0;
  return timeline.map(entry => {
    if (!isGenericToolName(entry.name)) return entry;
    const recovered = recoveredNames[recoveredIndex];
    if (!recovered) return entry;
    recoveredIndex += 1;
    return { ...entry, name: recovered };
  });
}

function mergedAssistantText(messages: readonly ThreadMessage[]): string {
  const texts = messages
    .map(message => unwrapToolCallEnvelope(message.content ?? '').text)
    .filter(text => text.trim().length > 0)
    .filter((text, index, all) => all.indexOf(text) === index);
  if (texts.length < 2) return texts[0] ?? '';

  // Legacy web delivery persisted both paragraph-sized segments and the full
  // final response. Prefer the complete response when it contains every
  // segment; otherwise retain each distinct assistant emission in order.
  const longest = [...texts].sort((left, right) => right.length - left.length)[0] ?? '';
  if (texts.every(text => longest.includes(text.trim()))) return longest;
  return texts.join('\n\n');
}

function mergeAssistantRun(messages: readonly ThreadMessage[]): ThreadMessage {
  if (messages.length === 1) return messages[0];
  const first = messages[0];
  const last = messages[messages.length - 1];
  const extraMetadata = Object.assign({}, ...messages.map(message => message.extraMetadata));
  const requestId = messages.map(requestIdOf).find(Boolean);
  const toolNames = messages.flatMap(
    message => unwrapToolCallEnvelope(message.content ?? '').toolNames
  );
  if (requestId) extraMetadata.requestId = requestId;
  if (toolNames.length > 0) extraMetadata[RECOVERED_TOOL_NAMES_KEY] = toolNames;
  // Defect B: this one visible message is several persisted rows, and the
  // feedback adapter is only ever handed the last row's id (`...last` below).
  // Carry the whole set so a rating written against that id stays attributable
  // to what the user actually saw, rather than to the final fragment of it.
  extraMetadata[FEEDBACK_ROW_IDS_METADATA_KEY] = messages.map(message => message.id);
  // A merged run inherits a rating from ANY of its rows: the row the adapter
  // wrote to is the last one, but an earlier persist (or a re-merge with
  // different boundaries) can leave it elsewhere.
  const merged = messages.map(persistedFeedback).find(Boolean);
  if (merged) extraMetadata[FEEDBACK_METADATA_KEY] = merged;
  return {
    ...last,
    content: mergedAssistantText(messages),
    createdAt: first.createdAt,
    extraMetadata,
  };
}

/**
 * A row the core persisted as a complete, self-contained delivery — an
 * background sub-agent result, a worker-thread hand-off, or a workflow proposal. Core
 * writers stamp `extraMetadata.scope` on every such row; the legacy segmented
 * path never did. That positive marker is what tells an async delivery apart
 * from a paragraph of the answer next to it, since neither carries a request
 * id on the wire.
 */
function isStandaloneDelivery(message: ThreadMessage): boolean {
  return typeof message.extraMetadata?.scope === 'string';
}

/**
 * Collapse legacy paragraph/tool-envelope rows into one assistant turn.
 *
 * The old interactive-web delivery path persisted each segment as a separate
 * agent message. Consecutive assistant rows cannot cross a user turn; when
 * both rows carry request ids, a differing id is the explicit boundary, and a
 * scoped standalone delivery ({@link isStandaloneDelivery}) is always its own
 * turn — it neither joins the run before it nor seeds the run after it.
 */
function coalesceAssistantSegments(messages: readonly ThreadMessage[]): ThreadMessage[] {
  const out: ThreadMessage[] = [];
  let run: ThreadMessage[] = [];
  let runRequestId: string | undefined;

  const flush = () => {
    if (run.length > 0) out.push(mergeAssistantRun(run));
    run = [];
    runRequestId = undefined;
  };

  for (const message of messages) {
    if (message.sender !== 'agent' || message.extraMetadata?.hidden) {
      flush();
      out.push(message);
      continue;
    }
    if (isStandaloneDelivery(message)) {
      flush();
      run.push(message);
      flush();
      continue;
    }
    const requestId = requestIdOf(message);
    if (run.length > 0 && runRequestId && requestId && runRequestId !== requestId) flush();
    run.push(message);
    runRequestId ??= requestId;
  }
  flush();
  return out;
}

function mimeTypeFromDataUri(dataUri: string): string {
  return dataUri.match(/^data:([^;,]+)/i)?.[1] ?? 'application/octet-stream';
}

function userParts(msg: ThreadMessage): ThreadUserMessagePart[] {
  const parsed = parseMessageImages(msg.content ?? '');
  const metadata = msg.extraMetadata ?? {};
  const kinds = stringArray(metadata.attachmentKinds);
  const names = stringArray(metadata.attachmentNames);
  const posters = stringArray(metadata.attachmentPosters);
  const metadataUris = stringArray(metadata.attachmentDataUris);
  const dataUris = metadataUris.length > 0 ? metadataUris : parsed.dataUris;
  const parts: ThreadUserMessagePart[] = [];

  if (parsed.text.length > 0) parts.push({ type: 'text', text: parsed.text });

  if (kinds.length === 0) {
    for (const [index, image] of dataUris.entries()) {
      parts.push({ type: 'image', image, filename: names[index] });
    }
    return parts;
  }

  for (const [index, kind] of kinds.entries()) {
    const filename = names[index];
    if (kind === 'image') {
      const image = dataUris[index];
      if (image) parts.push({ type: 'image', image, filename });
      continue;
    }
    if (kind === 'video') {
      const image = posters[index];
      if (image) parts.push({ type: 'image', image, filename });
      else parts.push({ type: 'file', filename, data: '', mimeType: 'video/mp4' });
      continue;
    }
    const data = dataUris[index] ?? '';
    parts.push({ type: 'file', filename, data, mimeType: mimeTypeFromDataUri(data) });
  }
  return parts;
}

export function toThreadMessageLike(
  msg: ThreadMessage,
  timeline: readonly ToolTimelineEntry[] = EMPTY_TIMELINE,
  transcript: readonly ProcessingTranscriptItem[] = EMPTY_TRANSCRIPT
): ThreadMessageLike {
  const cached = conversionCache.get(msg);
  if (cached?.timeline === timeline && cached.transcript === transcript) return cached.converted;

  const unwrapped = unwrapToolCallEnvelope(msg.content ?? '');
  const text = msg.sender === 'agent' ? unwrapped.text : (msg.content ?? '');
  const recoveredToolNames = [
    ...unwrapped.toolNames,
    ...stringArray(msg.extraMetadata?.[RECOVERED_TOOL_NAMES_KEY]),
  ];
  const effectiveTimeline = recoverTimelineToolNames(timeline, recoveredToolNames);
  const feedback = msg.sender === 'agent' ? persistedFeedback(msg) : undefined;

  const converted: ThreadMessageLike = {
    id: msg.id,
    role: msg.sender === 'agent' ? 'assistant' : 'user',
    content:
      msg.sender === 'agent' ? assistantParts(text, effectiveTimeline, transcript) : userParts(msg),
    createdAt: new Date(msg.createdAt),
    ...(msg.sender === 'agent' && msg.extraMetadata?.stopped === true
      ? { status: { type: 'incomplete' as const, reason: 'cancelled' as const } }
      : {}),
    metadata: {
      // Defect A (#6459-adjacent, but its own bug): the runtime writes
      // `submittedFeedback` onto its OWN repository copy when a thumb is
      // pressed, and we supply `messages` rather than `messageRepository` — so
      // the runtime rebuilds from this converter's output on every store update
      // (`external-store-thread-runtime-core.js`) and that write is discarded.
      // Re-emitting it from the persisted value is what makes a pressed thumb
      // survive the next turn, a thread switch and a reload. Without this the
      // control silently un-presses, which is worse than having no control.
      ...(feedback ? { submittedFeedback: { type: feedback } } : {}),
      custom: {
        extraMetadata: msg.extraMetadata ?? {},
        sourceType: msg.type,
      },
    },
  };

  conversionCache.set(msg, { timeline, transcript, converted });
  return converted;
}

/**
 * The live tail as a running assistant message.
 *
 * The tail is deliberately NOT part of `thread.messagesByThreadId` — Redux keeps
 * the settled transcript and the in-flight preview in separate slices, which is
 * exactly what keeps settled message identities stable while tokens land. Here
 * that separation is re-joined for the runtime's benefit: one fresh object per
 * token, and only that one object is ever re-converted.
 */
export function streamingTailMessage(
  streaming: StreamingAssistantState | null,
  timeline: readonly ToolTimelineEntry[] = EMPTY_TIMELINE,
  transcript: readonly ProcessingTranscriptItem[] = EMPTY_TRANSCRIPT,
  approval: PendingApproval | null = null
): ThreadMessageLike | null {
  if (!approval && !streaming && timeline.length === 0 && transcript.length === 0) return null;
  const text = streaming?.content ?? '';
  let parts = assistantParts(text, timeline, transcript);
  // The live reasoning block. Only when the transcript has not yet recorded a
  // `thinking` item for this turn — once it has, `assistantParts` above is
  // already emitting it in its proper place and this would double it.
  //
  // It goes FIRST because it is what the agent thought before it answered, and
  // `Reasoning` renders it expanded while it streams, then collapses it. A turn
  // that has so far produced only thinking now mints a tail rather than
  // nothing, which is the point: the block is the in-flight signal, alongside
  // `RunningStatus`.
  if (streaming?.thinking.trim()) {
    const hasTranscriptThinking = transcript.some(item => item.kind === 'thinking');
    if (!hasTranscriptThinking) {
      parts.unshift(
        reasoningPart(streaming.thinking, streaming.thinkingStartedAt, streaming.thinkingEndedAt)
      );
    }
  }
  if (approval) parts = withApproval(parts, approval);
  if (parts.length === 0) return null;
  return {
    id: STREAMING_TAIL_ID,
    role: 'assistant',
    content: parts,
    // A parked gate is not a running turn: it is a turn stopped on the user.
    // `requires-action` is what gives the gated tool part its own
    // `requires-action` status (a tool part with no result inherits the
    // message's), which is the state assistant-ui renders a decision on.
    status: approval ? { type: 'requires-action', reason: 'interrupt' } : { type: 'running' },
    metadata: { custom: { requestId: streaming?.requestId, streaming: true } },
  };
}

export type AssistantUiProjection = {
  /** Whether the synthetic live tail has an active core turn driving it. */
  isRunning?: boolean;
  /**
   * The thread's parked ApprovalGate request, if any. Present means the turn is
   * blocked on the user, and the tail is minted even when nothing else would
   * mint one — a parked gate with no prompt is the failure mode this closes.
   */
  pendingApproval?: PendingApproval | null;
  liveTimeline?: readonly ToolTimelineEntry[];
  liveTranscript?: readonly ProcessingTranscriptItem[];
  turnTimelines?: Readonly<Record<string, readonly ToolTimelineEntry[]>>;
  turnTranscripts?: Readonly<Record<string, readonly ProcessingTranscriptItem[]>>;
};

/**
 * The full thread as assistant-ui sees it: settled transcript, then the live
 * tail when one is in flight.
 *
 * Hidden messages are filtered the same way the transcript filters them, so the
 * runtime's view of the thread and the rendered view cannot disagree about what
 * the conversation contains.
 */
export function buildRuntimeMessages(
  messages: readonly ThreadMessage[],
  streaming: StreamingAssistantState | null,
  projection: AssistantUiProjection = {}
): ThreadMessageLike[] {
  // A parked approval outranks the lifecycle: `chat_done` has not fired (the
  // turn is stopped, not finished), but a snapshot race that reports the turn
  // settled must not swallow the only surface that can unblock it.
  const pendingApproval = projection.pendingApproval ?? null;
  const coalescedMessages = coalesceAssistantSegments(messages);
  const out: ThreadMessageLike[] = [];
  const claimedRequestIds = new Set(
    coalescedMessages.flatMap(message =>
      message.sender === 'agent' && typeof message.extraMetadata?.requestId === 'string'
        ? [message.extraMetadata.requestId]
        : []
    )
  );
  const projectedRequestIds = [
    ...new Set([
      ...Object.keys(projection.turnTimelines ?? {}),
      ...Object.keys(projection.turnTranscripts ?? {}),
    ]),
  ].filter(requestId => !claimedRequestIds.has(requestId));
  // Async acknowledgements/background deliveries can be persisted without
  // message-level request metadata. The transcript maps are chronological and
  // request-keyed, so unclaimed trails can be paired with unanchored agent
  // messages in order — but only when the two sets are the same size. With a
  // surplus of unanchored messages, positional pairing hands a later turn's
  // tools to an earlier trail-less answer and leaves the real answer bare;
  // rendering those trails nowhere is the lesser wrong.
  const unanchoredAgentCount = coalescedMessages.filter(
    message =>
      message.sender === 'agent' &&
      !message.extraMetadata?.hidden &&
      typeof message.extraMetadata?.requestId !== 'string'
  ).length;
  const pairOrphanTrails =
    projectedRequestIds.length > 0 && projectedRequestIds.length === unanchoredAgentCount;
  let orphanRequestCursor = 0;
  const lastVisibleAgentId = [...coalescedMessages]
    .reverse()
    .find(message => message.sender === 'agent' && !message.extraMetadata?.hidden)?.id;
  for (const msg of coalescedMessages) {
    if (msg.extraMetadata?.hidden) continue;
    const requestId =
      msg.sender === 'agent' && typeof msg.extraMetadata?.requestId === 'string'
        ? msg.extraMetadata.requestId
        : undefined;
    const effectiveRequestId =
      requestId ??
      (msg.sender === 'agent' && pairOrphanTrails
        ? projectedRequestIds[orphanRequestCursor++]
        : undefined);
    const persistedTimeline = effectiveRequestId
      ? projection.turnTimelines?.[effectiveRequestId]
      : undefined;
    const persistedTranscript = effectiveRequestId
      ? projection.turnTranscripts?.[effectiveRequestId]
      : undefined;
    // `chat_done` clears the active lifecycle before the completed snapshot is
    // indexed into the request maps. Keep the just-settled tools/reasoning on
    // the final assistant message during that handoff; never mint a running
    // synthetic tail for them.
    // Not while a gate is parked: the tail below is minted unconditionally in
    // that case and would emit the same rows a second time, and a repeated
    // `toolCallId` throws inside assistant-ui rather than dropping a row.
    const useSettledLiveFallback =
      projection.isRunning === false &&
      !pendingApproval &&
      msg.id === lastVisibleAgentId &&
      !persistedTimeline &&
      !persistedTranscript;
    out.push(
      toThreadMessageLike(
        msg,
        persistedTimeline ??
          (useSettledLiveFallback ? (projection.liveTimeline ?? EMPTY_TIMELINE) : EMPTY_TIMELINE),
        persistedTranscript ??
          (useSettledLiveFallback
            ? (projection.liveTranscript ?? EMPTY_TRANSCRIPT)
            : EMPTY_TRANSCRIPT)
      )
    );
  }
  const tail =
    projection.isRunning === false && !pendingApproval
      ? null
      : streamingTailMessage(
          streaming,
          projection.liveTimeline ?? EMPTY_TIMELINE,
          projection.liveTranscript ?? EMPTY_TRANSCRIPT,
          pendingApproval
        );
  if (tail) out.push(tail);
  return out;
}
