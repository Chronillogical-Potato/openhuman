---
description: >-
  Trust boundary for the autonomous core - autonomy / risk policy, the policy
  that decides when to sandbox (mechanism lives in tinybox), audit log,
  encrypted secret store, public-bind / pairing guard, a
icon: shield-halved
---

# Security

`crates/openhuman-core/src/security/` is the **trust boundary for the autonomous core**. It owns the autonomy / risk policy that decides whether a given tool call is allowed, the pluggable sandbox backends that confine those calls when the host supports it, the append-only audit log of every agent action, the encrypted secret store, the pairing guard that gates public binding of the RPC server, and the `redact()` helper every other domain uses to keep logs free of plaintext credentials.

It does **not** own:

* The cross-domain `EncryptionEngine`, which lives in `crates/openhuman-core/src/security/encryption/`.
* Per-channel credential storage, which lives in `crates/openhuman-core/src/security/credentials/`.

This module is the place to look first when asking "is this agent action allowed, and if so, how is it confined?"

## Public surface

| Item                                                                                                                          | File                                                                                                                                                | Purpose                                                                  |
| ----------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `SecurityPolicy`                                                                                                              | `policy/types.rs` (path checks in `policy/path_checks.rs`, command classification in `policy/command_checks.rs`, gating in `policy/enforcement.rs`) | Assembles runtime policy from `AutonomyConfig` + workspace dir.          |
| `AutonomyLevel` (`ReadOnly` / `Supervised` / `Full`)                                                                          | `policy/types.rs`                                                                                                                                   | Three-step autonomy ladder.                                              |
| `CommandRiskLevel`, `ToolOperation`, `ActionTracker`                                                                          | `policy/types.rs`                                                                                                                                   | Risk classification + per-session counting.                              |
| `SecretStore`                                                                                                                 | `keyring/encrypted_store.rs` (`secrets.rs` re-exports it)                                                                                           | OS-keychain / encrypted-file secret persistence with round-trip helpers. |
| `AuditLogger`, `AuditEventType`, `AuditEvent`, `Actor`, `Action`, `ExecutionResult`, `SecurityContext`, `CommandExecutionLog` | `audit.rs`                                                                                                                                          | Append-only audit trail.                                                 |
| `PairingGuard`, `constant_time_eq`, `is_public_bind`                                                                          | `pairing.rs` (`PairingGuard` and `constant_time_eq` are re-exported from `tinychannels_bus::security`)                                              | Pairing-token check before binding the RPC server publicly.              |
| `redact(value: &str) -> String`                                                                                               | `core.rs`                                                                                                                                           | Uniform 4-char-prefix redaction for logs.                                |
| `security_policy_info_for_config(&Config) -> Outcome<serde_json::Value>`                                                      | `ops.rs`                                                                                                                                            | RPC handler for the doctor / settings UI.                                |

## Sandboxing

This module no longer carries sandbox backends. The old `Sandbox` trait, `NoopSandbox`, `create_sandbox` and the Docker / Bubblewrap / Firejail / Landlock `Command` wrappers had no callers and were removed. Per-session sandbox selection lives in `crates/openhuman-core/src/sandbox/`, which delegates local OS confinement to `tinybox-jail` (vendored tinybox) and keeps its own `docker run` executor. Shell-string scanning used by command classification is `tinybox_core::shell::scan`; the classification rules stay here.

## Autonomy ladder

`AutonomyLevel` is a three-step ladder that controls how aggressively the policy gates tool calls, and it only takes effect while `[autonomy] enabled = true`. `ReadOnly` blocks every writing class outright: no in-tier approval can authorize one. `Supervised`, the default, allows reads and parks everything else for an approval round trip. `Full` allows reads and writes and still parks the network, install and destructive classes. The wire spellings are lowercase (`readonly`, `supervised`, `full`); the user-facing table is in [Approval Gate](../../features/approval-gate.md).

`CommandRiskLevel` and `ToolOperation` classify a given tool call; `ActionTracker` keeps the per-session counts that the policy compares against those caps. The agent harness asks `SecurityPolicy` for a decision before every executable tool dispatch.

All of this only applies when the autonomy policy is turned on. Per `AGENTS.md`, `[autonomy] enabled = false` is the default, and `SecurityPolicy::from_config` carries that flag through every enforcement entry point: with the policy disabled, command classification, the approval gate, the command allowlist, and the hourly action budget are all inert, and `workspace_only`, `forbidden_paths`, and the workspace-internal boundary are not enforced. One thing never turns off: `is_always_forbidden` blocks credential stores (`~/.ssh`, `~/.gnupg`, `~/.aws`) and system roots, and rejects `..` traversal and null bytes in a path, whether the autonomy policy is enabled or not. That is the floor this module keeps regardless of configuration.

## Audit log

`audit.rs` writes an append-only stream of `AuditEvent`s under the workspace dir. Every executable tool call lands here with its `Actor` (agent / user), `Action`, `ExecutionResult`, and the `SecurityContext` (autonomy level, sandbox backend, etc.) it ran under. The log is the post-hoc story of what the agent did and why it was allowed.

## Pairing guard

`PairingGuard` (in `pairing.rs`) stands between the RPC server and any attempt to bind to a non-loopback address. `is_public_bind` detects the dangerous case; `PairingGuard` requires a constant-time-compared pairing token (`constant_time_eq`) before such a bind is permitted. This is the iOS / LAN-companion pairing flow's defence against an unpaired peer attaching to the desktop core.

## Secret store

`SecretStore` (implemented in `keyring/encrypted_store.rs`, re-exported through `secrets.rs`) encrypts config-field secrets with ChaCha20-Poly1305 (`enc2:` prefix) under a keychain-backed master key, migrating the legacy XOR `enc:` format on decrypt. Backend selection and the encrypted-file fallback are described in `crates/openhuman-core/src/security/keyring/README.md`.

## `redact()`

`redact(value)` returns a uniform 4-char-prefix string (e.g. `"sk-a"` -> `"sk-a…"`) for use in logs and error messages. Use it whenever a secret, credential, token, or PII string is about to be formatted into a `log::` / `tracing::` call. Other domains call it directly: `credentials/`, `webhooks/`, `composio/`, the integration adapters.

## Layout

| Path                                                                                                                                | Role                                                                                                                     |
| ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `policy/` (`mod.rs`, `types.rs`, `path_checks.rs`, `command_checks.rs`, `enforcement.rs`, `policy_*_tests.rs`, `proptest_tests.rs`) | `SecurityPolicy`, `AutonomyLevel`, risk classification, path and command checks, action tracking.                        |
| `core.rs`, `core_tests.rs`                                                                                                          | `redact()` + small shared helpers.                                                                                       |
| `audit.rs`                                                                                                                          | Append-only audit log types.                                                                                             |
| `secrets.rs`, `keyring/`                                                                                                            | `SecretStore` (implemented in `keyring/encrypted_store.rs`) + round-trip tests.                                          |
| `pairing.rs`, `pairing_tests.rs`                                                                                                    | `PairingGuard` + constant-time helpers.                                                                                  |
| `ops.rs`                                                                                                                            | RPC handler (`security_policy_info_for_config`).                                                                         |
| `schemas.rs`                                                                                                                        | Controller schemas + handler dispatch.                                                                                   |
| `mod.rs`                                                                                                                            | Re-exports of the public surface above.                                                                                  |
| `live_policy.rs`, `scrub.rs`, `tools.rs`                                                                                            | Live-policy lookup, the host scrubbing policy (`scrub::host_policy`), and the domain's own agent tools.                  |
| `approval/`, `credentials/`, `devices/`, `egress/`, `encryption/`, `keyring_consent/`, `pii/`, `prompt_injection/`                  | Sibling kernel-security domains under the same module. Each owns its own surface; this page covers the policy core only. |

## Calls into

* `crates/openhuman-core/src/config/`: `SecurityConfig`, `AutonomyConfig` for policy + sandbox selection.
* Workspace filesystem, for the audit log and secret store.

## Called by

* `crates/openhuman-core/src/cron/ops.rs`: wraps shell jobs in `SecurityPolicy::from_config`.
* `crates/openhuman-core/src/tools/ops.rs` and `tools/impl/{browser,document,filesystem,network,presentation,system}/`: every executable tool consults `SecurityPolicy`.
* `crates/openhuman-core/src/tools/impl/network/{gate,mcp,mcp_server_tools}.rs`: risk-classify outbound calls.
* `crates/openhuman-core/src/memory/guard.rs`: wraps every memory write in `security::scrub::host_policy()`.
* `crates/openhuman-core/src/agent/tools/delegate.rs`: sub-agent dispatch goes through the autonomy gate.
* `crates/openhuman-core/src/security/credentials/`: uses `SecretStore` and `redact`.

## Tests

* Unit: `pairing_tests.rs`, `policy/policy_tests*.rs`, `policy/proptest_tests.rs`, `keyring/encrypted_store_tests*.rs`.
* `core_tests.rs` covers `redact()`.

## Related

* [`security/README.md`](https://github.com/tinyhumansai/openhuman/blob/main/crates/openhuman-core/src/security/README.md): authoritative internal-audience overview this page mirrors.
* [Architecture overview](architecture.md): wider system context.
* [Agent Harness](agent-harness.md): where `SecurityPolicy` is consulted on every tool dispatch.
