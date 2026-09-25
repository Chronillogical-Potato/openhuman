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

/**
 * Start the mock backend. Signature per `scripts/mock-api/server.mjs:254`:
 * port first, options second.
 *
 * The result has two shapes and this declaration has to admit both, so the
 * extra fields are optional rather than absent: an already-running server
 * returns `{ port, alreadyRunning: true }` (`server.mjs:256`), while a fresh
 * start also reports the port it was asked for and whether `retryIfInUse` had
 * to move off it (`server.mjs:286-290`). An earlier version of this file
 * declared only the first shape, which compiled everywhere until a test read
 * `requestedPort` — the same failure mode the `MockRequestEntry` note above
 * describes, in the opposite direction: a declaration NARROWER than the runtime
 * hides fields that exist, rather than inventing ones that do not.
 */
export function startMockServer(
  port?: number,
  options?: { retryIfInUse?: boolean },
): Promise<{
  port: number;
  alreadyRunning?: boolean;
  /** The port asked for, when a fresh start resolved one. */
  requestedPort?: number;
  /** True when `retryIfInUse` moved the server off `requestedPort`. */
  retried?: boolean;
}>;

export function stopMockServer(): Promise<void>;

export function emitMockAgentAudioStream(
  payload: Record<string, unknown>,
): unknown;
