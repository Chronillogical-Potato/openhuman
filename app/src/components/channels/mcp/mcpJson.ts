/**
 * Reading and checking the `mcp.json` a user types into the editor
 * (`McpJsonEditor.tsx`).
 *
 * Split out of the component because these are the rules a text editor needs
 * and a form does not: a form can only produce shapes the core accepts, while a
 * document can say anything, and the difference between "this isn't JSON",
 * "this isn't the shape" and "the core refused it" is the difference between a
 * user fixing a typo in three seconds and staring at a red box. The core
 * validates it all again — it must, it is the authority — but a round trip to
 * learn a brace is missing is a round trip the editor can save.
 */
import type { McpConfigDoc, McpConfigEntry } from './types';

/** The fields the core echoes for a reader and ignores on write. */
const ECHOED_FIELDS = ['envKeys', 'authConfigured'] as const;

/**
 * A parse that produced a document, or the reason it didn't. `code` is the
 * stable i18n suffix (`mcp.json.parseError.<code>`); `name` and `detail` fill
 * the message's placeholders when it has them.
 */
export type McpJsonParse =
  | { ok: true; doc: McpConfigDoc }
  | { ok: false; code: McpJsonParseErrorCode; name?: string; detail?: string };

export type McpJsonParseErrorCode =
  | 'empty'
  | 'invalidJson'
  | 'rootNotObject'
  | 'missingRoot'
  | 'rootNotMap'
  | 'emptyName'
  | 'entryNotObject'
  | 'needsUrlOrCommand'
  | 'bothUrlAndCommand';

/** The document as text, in the shape the editor shows it. */
export function formatMcpConfig(doc: McpConfigDoc): string {
  return `${JSON.stringify(doc, null, 2)}\n`;
}

/** The document an MCP-less setup is. */
export const EMPTY_MCP_CONFIG: McpConfigDoc = { mcpServers: {} };

/**
 * Reads the editor's text as an `mcp.json`.
 *
 * Checks only what a client can check without guessing at the core's rules:
 * that it is JSON, that it has the `mcpServers` object, and that each entry is
 * an object naming exactly one of `url` or `command`. Everything else —
 * whether the command exists, whether a field is one the core stores — is the
 * core's answer to give, and the editor shows it verbatim rather than
 * second-guessing it here.
 *
 * The echoed fields (`envKeys`, `authConfigured`) are stripped from the result.
 * The core ignores them, but sending back a flag the user may have edited
 * invites the belief that editing it does something.
 */
export function parseMcpConfig(text: string): McpJsonParse {
  if (!text.trim()) {
    return { ok: false, code: 'empty' };
  }
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (err) {
    return { ok: false, code: 'invalidJson', detail: err instanceof Error ? err.message : '' };
  }
  if (!isRecord(value)) {
    return { ok: false, code: 'rootNotObject' };
  }
  const servers = value.mcpServers;
  if (servers === undefined) {
    return { ok: false, code: 'missingRoot' };
  }
  if (!isRecord(servers)) {
    return { ok: false, code: 'rootNotMap' };
  }
  const out: Record<string, McpConfigEntry> = {};
  for (const [name, entry] of Object.entries(servers)) {
    if (!name.trim()) {
      return { ok: false, code: 'emptyName' };
    }
    if (!isRecord(entry)) {
      return { ok: false, code: 'entryNotObject', name };
    }
    const hasUrl = typeof entry.url === 'string' && entry.url.trim().length > 0;
    const hasCommand = typeof entry.command === 'string' && entry.command.trim().length > 0;
    if (hasUrl && hasCommand) {
      return { ok: false, code: 'bothUrlAndCommand', name };
    }
    if (!hasUrl && !hasCommand) {
      return { ok: false, code: 'needsUrlOrCommand', name };
    }
    const clean = { ...entry } as Record<string, unknown>;
    for (const field of ECHOED_FIELDS) delete clean[field];
    out[name] = clean as McpConfigEntry;
  }
  return { ok: true, doc: { mcpServers: out } };
}

/**
 * Whether the text says anything the loaded document does not.
 *
 * Compares the *parsed* documents, so reformatting, key reordering and
 * whitespace are not edits — a Save button lit by a stray newline trains a
 * user to ignore it.
 */
export function mcpConfigChanged(text: string, loaded: McpConfigDoc | null): boolean {
  if (loaded === null) return text.trim().length > 0;
  const parsed = parseMcpConfig(text);
  if (!parsed.ok) return true;
  const stripped = parseMcpConfig(formatMcpConfig(loaded));
  if (!stripped.ok) return true;
  return stableJson(parsed.doc) !== stableJson(stripped.doc);
}

/** JSON with object keys sorted, so key order is not a difference. */
function stableJson(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(',')}]`;
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map(key => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value) ?? 'null';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
