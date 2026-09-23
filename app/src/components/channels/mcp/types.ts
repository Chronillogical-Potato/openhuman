/**
 * Shared TypeScript types for the MCP Servers tab.
 * Single source of truth — import from here, not from the API layer.
 */

export type SmitheryServer = {
  qualified_name: string;
  display_name: string;
  description?: string;
  icon_url?: string;
  use_count?: number;
  is_deployed?: boolean;
  /**
   * Upstream registry this row came from — `'mcp_official'` (the official
   * modelcontextprotocol.io registry) or `'smithery'`. Stamped by the Rust
   * dispatcher; used to attribute each row to its source registry.
   */
  source?: string;
  /**
   * `true` when this is the canonical first-party server for a well-known
   * service (exact `qualified_name` match server-side). The UI badges it
   * "Official"; every other server is shown without a badge — nothing is
   * hidden. Stamped by the Rust dispatcher; never trusted from the wire.
   */
  official?: boolean;
  /**
   * Vendor/site URL declared by the server, when present. The strict catalog
   * filter requires it, so every listed row carries one; rendered as a
   * clickable external link.
   */
  website_url?: string;
  /**
   * Declared auth method from registry metadata. `'api_key'` means the server
   * declares a named static secret (header/env). The strict filter only lists
   * `'api_key'` servers, so connecting never depends on a probe guess.
   */
  auth_kind?: 'api_key' | string;
};

export type SmitheryConnection = {
  type: 'stdio' | 'http';
  deployment_url?: string;
  config_schema?: unknown;
  example_config?: unknown;
  published?: boolean;
};

export type SmitheryServerDetail = SmitheryServer & {
  connections: SmitheryConnection[];
  required_env_keys?: string[];
};

export type CommandKind = 'node' | 'python' | 'binary';

/** How an installed server is dialled: a local subprocess or a hosted endpoint. */
export type InstalledTransport = { kind: 'stdio' } | { kind: 'http_remote'; url: string };

export type InstalledServer = {
  server_id: string;
  qualified_name: string;
  display_name: string;
  description?: string;
  icon_url?: string;
  command_kind: CommandKind;
  command: string;
  args: string[];
  env_keys: string[];
  config?: unknown;
  installed_at: number;
  last_connected_at?: number;
  /** Absent on rows persisted before the field existed; those are stdio. */
  transport?: InstalledTransport;
  enabled: boolean;
};

/**
 * One server as `mcp.json` declares it. Exactly one of `url` (hosted) or
 * `command` (run locally) is set. `env` (local) / `headers` (hosted) are
 * write-only credentials: accepted on save, never present on a read. A read
 * carries `envKeys` (the stored names) and `authConfigured` instead; both are
 * ignored on save.
 */
export type McpConfigEntry = {
  url?: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  headers?: Record<string, string>;
  description?: string;
  enabled?: boolean;
  envKeys?: string[];
  authConfigured?: boolean;
  /** Tolerated for documents copied from other clients; ignored. */
  type?: string;
};

/** The document: `{ "mcpServers": { … } }`. */
export type McpConfigDoc = { mcpServers: Record<string, McpConfigEntry> };

/** What a save answers: the re-rendered document plus what changed. */
export type McpConfigWriteResult = McpConfigDoc & {
  added: string[];
  updated: string[];
  removed: string[];
};

export type McpTool = { name: string; description?: string; input_schema: unknown };

export type ServerStatus =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  // Server reachable but rejected the connect with HTTP 401 — needs sign-in or
  // an access token. Distinct from `error` so the UI offers a re-auth path.
  | 'unauthorized'
  | 'error'
  | 'disabled';

export type ConnStatus = {
  server_id: string;
  qualified_name: string;
  display_name: string;
  status: ServerStatus;
  tool_count: number;
  last_error?: string;
  /**
   * Stable auth-failure reason code refining `status === 'unauthorized'`:
   * `'oauth_required'` / `'token_rejected'` / `'credential_required'`. Present
   * only for a 401; never carries the raw message / OAuth metadata URL (#4289).
   */
  auth_hint?: string;
};
