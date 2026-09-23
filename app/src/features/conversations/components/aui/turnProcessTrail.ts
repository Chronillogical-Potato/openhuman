/**
 * Read the settled turn's `TurnProcessTrail` off assistant-ui's message
 * metadata.
 *
 * Shared by the two consumers of that trail — `TurnFooter` (the `8 steps ·
 * 2 tools` door) and `TurnSources` (the inline source list) — so both agree on
 * what counts as a usable trail.
 */
import type { TurnProcessTrail } from '../../../../providers/assistantUiMessages';

/**
 * Narrow the runtime's untyped `message.metadata.custom` back to our own shape.
 *
 * `custom` is `unknown` by contract and either consumer can be mounted on a
 * message this projection did not build (the kit's own demo runtime, a test
 * harness), so this returns `null` rather than asserting.
 *
 * **Every field is checked, including the two arrays.** The previous version
 * validated `steps` and `tools` and then cast, which asserted more than it had
 * proven: a caller reading `trail.timeline` got a typed array that the
 * validator had never looked at. `TurnSources` reads exactly that field, so the
 * gap stopped being theoretical.
 */
export function readProcessTrail(metadata: unknown): TurnProcessTrail | null {
  if (typeof metadata !== 'object' || metadata === null) return null;
  const custom = (metadata as { custom?: unknown }).custom;
  if (typeof custom !== 'object' || custom === null) return null;
  const trail = (custom as { processTrail?: unknown }).processTrail;
  if (typeof trail !== 'object' || trail === null) return null;
  const candidate = trail as Partial<TurnProcessTrail>;
  if (typeof candidate.steps !== 'number' || typeof candidate.tools !== 'number') return null;
  if (!Array.isArray(candidate.timeline) || !Array.isArray(candidate.transcript)) return null;
  return candidate as TurnProcessTrail;
}
