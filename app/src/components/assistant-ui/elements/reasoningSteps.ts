/**
 * Turns a raw reasoning trace into the titled steps the static
 * `ReasoningPanel` renders, and formats the "Thought for Ns" resting label.
 *
 * Reasoning arrives as free text. Providers that summarise their reasoning
 * (OpenAI / Codex style) mark each section with a bold line — `**Planning the
 * fix**` — or a markdown heading; each such line starts a step. Text with no
 * headings is split into paragraph groups and each is titled by its first
 * sentence, so every trace renders as steps rather than one opaque block.
 */

export interface ReasoningStep {
  title: string;
  body: string;
  /**
   * True when the title was derived from the text (no heading marked it).
   * A derived title keeps changing while its sentence streams in, so it is
   * never used as the live trigger label.
   */
  derived?: boolean;
}

/** Per-part timing carried on a reasoning part's `providerMetadata.openhuman`. */
export interface ReasoningTiming {
  startedAt?: number;
  endedAt?: number;
}

const BOLD_HEADING = /^\s*\*\*([^*\n]+?)\*\*\s*:?\s*$/;
const MD_HEADING = /^\s{0,3}#{1,4}\s+(.+?)\s*#*\s*$/;
/** Longest derived (untitled) step title, in characters. */
export const DERIVED_TITLE_MAX = 60;

function headingOf(line: string): string | null {
  const bold = BOLD_HEADING.exec(line);
  if (bold?.[1]) return bold[1].trim();
  const md = MD_HEADING.exec(line);
  if (md?.[1]) return md[1].replace(/\*\*/g, '').trim();
  return null;
}

/**
 * Title an untitled paragraph by its first sentence, trimmed on a word
 * boundary. The body keeps whatever the title did not consume, so no text is
 * ever dropped from the trace.
 */
function deriveStep(paragraph: string): ReasoningStep {
  const text = paragraph.trim();
  const sentenceEnd = /[.!?](\s|$)/.exec(text);
  const firstSentence = sentenceEnd ? text.slice(0, sentenceEnd.index + 1) : text;
  const firstLine = firstSentence.split('\n')[0] ?? firstSentence;
  const plain = firstLine.replace(/[*_`]/g, '').trim();

  if (plain.length <= DERIVED_TITLE_MAX && firstLine.length === firstSentence.length) {
    const rest = text.slice(firstSentence.length).trim();
    return { title: plain.replace(/[.:]$/, ''), body: rest, derived: true };
  }
  // Too long (or the sentence spans lines): shorten for the title and keep the
  // whole paragraph as the body.
  const cut = plain.slice(0, DERIVED_TITLE_MAX);
  const lastSpace = cut.lastIndexOf(' ');
  const title = `${(lastSpace > 20 ? cut.slice(0, lastSpace) : cut).replace(/[,.;:]$/, '')}…`;
  return { title, body: text, derived: true };
}

function splitParagraphs(text: string): string[] {
  return text
    .split(/\n\s*\n/)
    .map(p => p.trim())
    .filter(p => p.length > 0);
}

/**
 * Parse one reasoning trace into steps. Headed sections become one step each;
 * text before the first heading (or a trace with no headings at all) becomes
 * one derived step per paragraph. A heading still streaming in (`**Plan` with
 * no closing marker yet) does not match and stays body text until it closes.
 */
export function parseReasoningSteps(text: string): ReasoningStep[] {
  if (!text.trim()) return [];
  const steps: ReasoningStep[] = [];
  const state: { current: { title: string; lines: string[] } | null; preamble: string[] } = {
    current: null,
    preamble: [],
  };

  const flush = () => {
    if (state.current) {
      steps.push({ title: state.current.title, body: state.current.lines.join('\n').trim() });
      state.current = null;
    } else if (state.preamble.length > 0) {
      for (const p of splitParagraphs(state.preamble.join('\n'))) steps.push(deriveStep(p));
    }
    state.preamble = [];
  };

  for (const line of text.split('\n')) {
    const heading = headingOf(line);
    if (heading) {
      flush();
      state.current = { title: heading, lines: [] };
    } else if (state.current) {
      state.current.lines.push(line);
    } else {
      state.preamble.push(line);
    }
  }
  flush();
  return steps;
}

/**
 * The label to show while the trace streams: the newest heading the model
 * wrote (Codex style), or `undefined` when it has written none yet.
 */
export function latestHeading(steps: readonly ReasoningStep[]): string | undefined {
  for (let i = steps.length - 1; i >= 0; i -= 1) {
    const step = steps[i];
    if (step && !step.derived) return step.title;
  }
  return undefined;
}

/** Parse several reasoning parts (one per model round) into one step list. */
export function parseReasoningParts(texts: readonly string[]): ReasoningStep[] {
  return texts.flatMap(parseReasoningSteps);
}

/**
 * The span a group of reasoning parts covers: earliest start to latest end.
 * `undefined` when no part carries timing (threads recorded before timing).
 */
export function reasoningSpan(
  timings: readonly (ReasoningTiming | undefined)[]
): { startedAt: number; endedAt: number | undefined } | undefined {
  let startedAt: number | undefined;
  let endedAt: number | undefined;
  for (const timing of timings) {
    if (!timing) continue;
    if (typeof timing.startedAt === 'number') {
      startedAt = startedAt === undefined ? timing.startedAt : Math.min(startedAt, timing.startedAt);
    }
    if (typeof timing.endedAt === 'number') {
      endedAt = endedAt === undefined ? timing.endedAt : Math.max(endedAt, timing.endedAt);
    }
  }
  if (startedAt === undefined) return undefined;
  return { startedAt, endedAt };
}

type Translate = (key: string) => string;

/** A compact duration: `12s`, `1m 12s`. */
export function formatElapsed(ms: number, t: Translate): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  if (total < 60) return t('chat.reasoning.elapsedSeconds').replace('{n}', String(total));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return t('chat.reasoning.elapsedMinutes').replace('{m}', String(m)).replace('{s}', String(s));
}

/**
 * The settled label: "Thought for 12s", "Thought briefly" under a second, or
 * a plain "Thought" when the duration is unknown.
 */
export function thoughtForLabel(durationMs: number | undefined, t: Translate): string {
  if (durationMs === undefined || !Number.isFinite(durationMs) || durationMs < 0) {
    return t('chat.reasoning.thought');
  }
  if (durationMs < 1000) return t('chat.reasoning.thoughtBriefly');
  return t('chat.reasoning.thoughtFor').replace('{n}', formatElapsed(durationMs, t));
}
