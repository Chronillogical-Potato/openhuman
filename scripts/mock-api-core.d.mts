/**
 * Types for the mock backend shim — declarations only, no runtime.
 *
 * `mock-api-core.mjs` is plain JavaScript, so every consumer of it imported
 * `any`. That is invisible in a spec until a `.find(r => ...)` callback turns
 * into eleven identical TS7006s the moment anything type-checks the e2e trees,
 * which is what `pnpm --filter openhuman-app typecheck:e2e` now does.
 *
 * Declared here rather than as a wrapper in `app/test/e2e/mock-server.ts`:
 * that file leads with `// @ts-nocheck`, and adding an import to it puts
 * Prettier's import sorter above that directive, which silently disables it for
 * the whole file. A sibling declaration file has no such ordering hazard and
 * types every consumer of the module, not just the ones that go through the app's
 * wrapper.
 */

/**
 * One entry in the mock backend's request log.
 *
 * Mirrors `scripts/mock-api/server.mjs:72-78` field for field, and deliberately
 * no wider: a field the server never records would type-check at the call site
 * and be `undefined` at runtime. `connector-gmail-composio.spec.ts` was logging
 * exactly such a field (`statusCode`) before this existed.
 */
export interface MockRequestEntry {
  method: string;
  url: string;
  /** Raw body, still a string — callers `JSON.parse` it themselves. */
  body: string;
  headers: Record<string, string>;
  timestamp: number;
}

export const DEFAULT_PORT: number;

export function clearRequestLog(): void;
/** Every request served since the last reset, oldest first. */
export function getRequestLog(): MockRequestEntry[];

export function getMockBehavior(): Record<string, string>;
export function setMockBehavior(key: string, value: string): void;
export function setMockBehaviors(
  behavior: Record<string, string>,
  mode?: "merge" | "replace",
): void;
export function resetMockBehavior(): void;

export function getMockServerPort(): number | null;

/** Signature per `scripts/mock-api/server.mjs:254`: port first, options second. */
export function startMockServer(
  port?: number,
  options?: { retryIfInUse?: boolean },
): Promise<{ port: number; alreadyRunning?: boolean }>;

export function stopMockServer(): Promise<void>;

export function emitMockAgentAudioStream(
  payload: Record<string, unknown>,
): unknown;
